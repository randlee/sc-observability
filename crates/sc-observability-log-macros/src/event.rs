//! Expansion of a parsed [`EventSpec`] into a guarded `emit_callsite` call.
//!
//! The expansion references only `::sc_observability_log::__private` and
//! `::core` / `::std` paths, so a consumer needs a single dependency. Labels are
//! not touched here: `target:` / `name:` and `{ KEY }` expressions are placed
//! unchanged into `static` `Callsite` / `DynamicKey` values and labelled at
//! runtime on first use.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::fields::{EventSpec, Field, FieldKey, FieldValue};

/// The implied level of a leveled macro.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ImpliedLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl ImpliedLevel {
    /// The `sc_observability_types::Level` variant name.
    fn variant(self) -> syn::Ident {
        let name = match self {
            ImpliedLevel::Trace => "Trace",
            ImpliedLevel::Debug => "Debug",
            ImpliedLevel::Info => "Info",
            ImpliedLevel::Warn => "Warn",
            ImpliedLevel::Error => "Error",
        };
        syn::Ident::new(name, Span::call_site())
    }
}

/// How the level of an expansion is obtained.
#[derive(Clone, Copy)]
pub(crate) enum LevelSource<'a> {
    /// `trace!` .. `error!`.
    Implied(ImpliedLevel),
    /// `event!`: the level expression, converted with `From`.
    Expression(&'a syn::Expr),
}

/// Expands one event macro.
pub(crate) fn expand(spec: &EventSpec, source: LevelSource<'_>) -> TokenStream {
    let private = quote!(::sc_observability_log::__private);

    let target = spec
        .target
        .as_ref()
        .map_or_else(|| quote!(::core::module_path!()), |target| quote!(#target));
    let action = spec.name.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |name| quote!(::core::option::Option::Some(#name)),
    );

    let level = match source {
        LevelSource::Implied(level) => {
            let variant = level.variant();
            quote!(#private::Level::#variant)
        }
        LevelSource::Expression(level) => quote_spanned! {level.span()=>
            <#private::Level as ::core::convert::From<_>>::from(#level)
        },
    };

    let has_bare = spec
        .fields
        .iter()
        .any(|field| matches!(field.value, FieldValue::Bare(_)));
    let kind_imports = has_bare.then(|| {
        quote! {
            #[allow(unused_imports)]
            use #private::{DebugKindTag as _, SerializeKindTag as _};
        }
    });

    let fields_ident = syn::Ident::new("__sc_fields", Span::mixed_site());
    let fields_decl = if spec.fields.is_empty() {
        quote!(let #fields_ident = #private::Map::new();)
    } else {
        quote!(let mut #fields_ident = #private::Map::new();)
    };
    let field_stmts = spec
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| expand_field(field, index, &fields_ident, &private));

    let message = spec.message.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |tail| quote!(::core::option::Option::Some(::std::format!(#tail))),
    );

    let callsite = syn::Ident::new("__SC_CALLSITE", Span::mixed_site());
    let level_ident = syn::Ident::new("__sc_level", Span::mixed_site());
    quote! {
        {
            static #callsite: #private::Callsite = #private::Callsite::new(#target, #action);
            let #level_ident: #private::Level = #level;
            if #private::enabled(#level_ident) {
                #kind_imports
                #fields_decl
                #(#field_stmts)*
                #private::emit_callsite(&#callsite, #level_ident, #message, #fields_ident);
            }
        }
    }
}

/// Expands one field into a statement inserting it into the field map.
pub(crate) fn expand_field(
    field: &Field,
    index: usize,
    fields_ident: &syn::Ident,
    private: &TokenStream,
) -> TokenStream {
    let record = expand_value(&field.value, private);
    match &field.key {
        FieldKey::Static { key, span } => {
            let key = syn::LitStr::new(key, *span);
            quote! {
                #private::record_field(&mut #fields_ident, #key, #record);
            }
        }
        FieldKey::Dynamic(key) => {
            let key_static = syn::Ident::new(&format!("__SC_KEY_{index}"), Span::mixed_site());
            let key = key.as_ref();
            quote! {
                {
                    static #key_static: #private::DynamicKey = #private::DynamicKey::new(#key);
                    #private::record_dynamic_field(&mut #fields_ident, &#key_static, #record);
                }
            }
        }
    }
}

/// Expands a field value into a `FieldRecord` expression.
fn expand_value(value: &FieldValue, private: &TokenStream) -> TokenStream {
    let expr = value.expr();
    match value {
        FieldValue::Debug(_) => quote!(#private::debug_value(&(#expr))),
        FieldValue::Display(_) => quote!(#private::display_value(&(#expr))),
        // Autoref kind selection; spanned on the field expression so the
        // `FieldDebug` diagnostic points at it.
        FieldValue::Bare(_) => quote_spanned! {expr.span()=>
            {
                let __sc_value = &(#expr);
                (&&#private::FieldValue(__sc_value)).__sc_field_kind().record(__sc_value)
            }
        },
    }
}
