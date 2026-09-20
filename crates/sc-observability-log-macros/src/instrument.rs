//! `#[instrument]`: argument parsing and the sync / async function rewrite.
//!
//! The argument grammar follows `tracing-attributes` 0.1.31 (`attr.rs`). The
//! rewrite keeps the signature, visibility, generics, `where` clause, attributes
//! and `async`-ness of the function and replaces only its body:
//!
//! - a `static` `Callsite` holds the raw `target` and `name` values, labelled at
//!   runtime by `sc-observability-log` (this crate never rewrites labels);
//! - a `CallSpan` records the arguments and `fields(..)` and owns the trace
//!   context;
//! - a sync body runs as `(move || body)()` inside one entered section, so
//!   `return` and `?` leave only the closure and the completion match always runs;
//! - an async body is pinned on the stack with `core::pin::pin!` and driven by
//!   `core::future::poll_fn`, re-entering the context on every poll (no `unsafe`,
//!   no allocation, no public future type).

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote, quote_spanned};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    AttrStyle, Expr, FnArg, GenericArgument, Ident, ItemFn, LitInt, LitStr, Pat, PathArguments,
    ReturnType, Token, Type,
};

use crate::event::expand_field;
use crate::fields::{Field, FieldContext, FieldKey, FieldValue, parse_field_list, unraw};

/// How `ret` / `err` values are formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FormatMode {
    /// Bare `ret` (Debug) or bare `err` (Display).
    #[default]
    Default,
    /// `(Debug)`.
    Debug,
    /// `(Display)`.
    Display,
}

/// A level argument.
#[derive(Clone)]
enum LevelArg {
    /// `"info"` (case-insensitive) or `1..=5`: the `sc_observability_types::Level` variant name.
    Named(&'static str),
    /// `Level::INFO` or a `const` path of type `sc_observability_log::Level`.
    Path(syn::Path),
}

impl LevelArg {
    fn tokens(&self, private: &TokenStream) -> TokenStream {
        match self {
            LevelArg::Named(variant) => {
                let variant = Ident::new(variant, Span::call_site());
                quote!(#private::Level::#variant)
            }
            LevelArg::Path(path) => quote_spanned! {path.span()=>
                <#private::Level as ::core::convert::From<_>>::from(#path)
            },
        }
    }
}

impl Parse for LevelArg {
    /// Parses the value after `level =`.
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        const UNKNOWN: &str = "unknown verbosity level, expected one of \"trace\", \"debug\", \"info\", \"warn\", or \"error\", or a number 1-5";
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            let value = lit.value();
            let named = ["Trace", "Debug", "Info", "Warn", "Error"]
                .into_iter()
                .find(|name| name.eq_ignore_ascii_case(&value));
            named
                .map(LevelArg::Named)
                .ok_or_else(|| syn::Error::new(lit.span(), UNKNOWN))
        } else if lookahead.peek(LitInt) {
            let lit: LitInt = input.parse()?;
            let named = match lit.base10_parse::<u8>() {
                Ok(1) => Some("Trace"),
                Ok(2) => Some("Debug"),
                Ok(3) => Some("Info"),
                Ok(4) => Some("Warn"),
                Ok(5) => Some("Error"),
                _ => None,
            };
            named
                .map(LevelArg::Named)
                .ok_or_else(|| syn::Error::new(lit.span(), UNKNOWN))
        } else if input.peek(Ident::peek_any) || input.peek(Token![::]) {
            input.parse().map(LevelArg::Path)
        } else {
            Err(lookahead.error())
        }
    }
}

/// `ret(..)` / `err(..)` arguments.
#[derive(Clone, Default)]
struct EventArgs {
    mode: FormatMode,
    level: Option<LevelArg>,
}

impl EventArgs {
    /// Parses the optional parenthesized `(Debug | Display, level = L)` list.
    fn parse_optional(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = EventArgs::default();
        if !input.peek(syn::token::Paren) {
            return Ok(args);
        }
        let content;
        syn::parenthesized!(content in input);
        while !content.is_empty() {
            let ident = Ident::parse_any(&content)?;
            if ident == "level" {
                if args.level.is_some() {
                    return Err(syn::Error::new(
                        ident.span(),
                        "expected only a single `level` argument",
                    ));
                }
                content.parse::<Token![=]>()?;
                args.level = Some(content.parse()?);
            } else if args.mode != FormatMode::Default {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected only a single format argument",
                ));
            } else if ident == "Debug" {
                args.mode = FormatMode::Debug;
            } else if ident == "Display" {
                args.mode = FormatMode::Display;
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "unknown event formatting mode, expected either `Debug` or `Display`",
                ));
            }
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

/// Parsed `#[instrument(..)]` arguments.
#[derive(Default)]
struct InstrumentArgs {
    name: Option<TokenStream>,
    target: Option<TokenStream>,
    level: Option<LevelArg>,
    skips: Vec<Ident>,
    skip_all: bool,
    fields: Option<Vec<Field>>,
    ret: Option<EventArgs>,
    err: Option<EventArgs>,
}

/// Parses `= "literal"` or `= CONST_PATH` after `name` / `target`.
fn parse_str_value(input: ParseStream<'_>) -> syn::Result<TokenStream> {
    input.parse::<Token![=]>()?;
    if input.peek(LitStr) {
        return Ok(input.parse::<LitStr>()?.into_token_stream());
    }
    Ok(input.parse::<syn::Path>()?.into_token_stream())
}

/// The expression `ident`, recording a parameter binding.
fn ident_expr(ident: &Ident) -> Expr {
    Expr::Path(syn::ExprPath {
        attrs: Vec::new(),
        qself: None,
        path: syn::Path::from(ident.clone()),
    })
}

fn duplicate(ident: &Ident) -> syn::Error {
    syn::Error::new(
        ident.span(),
        format!("expected only a single `{ident}` argument"),
    )
}

impl Parse for InstrumentArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = InstrumentArgs::default();
        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                if args.name.is_some() {
                    return Err(syn::Error::new(
                        lit.span(),
                        "expected only a single `name` argument",
                    ));
                }
                args.name = Some(lit.into_token_stream());
            } else {
                let ident = Ident::parse_any(input)?;
                args.parse_named(&ident, input)?;
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

impl InstrumentArgs {
    /// Parses one `ident ..` argument.
    fn parse_named(&mut self, ident: &Ident, input: ParseStream<'_>) -> syn::Result<()> {
        match ident.to_string().as_str() {
            "name" => set_once(&mut self.name, ident, parse_str_value(input)?),
            "target" => set_once(&mut self.target, ident, parse_str_value(input)?),
            "level" => {
                input.parse::<Token![=]>()?;
                set_once(&mut self.level, ident, input.parse()?)
            }
            "skip" => {
                if !self.skips.is_empty() {
                    return Err(duplicate(ident));
                }
                if self.skip_all {
                    return Err(syn::Error::new(
                        ident.span(),
                        "expected either `skip` or `skip_all` argument",
                    ));
                }
                let content;
                syn::parenthesized!(content in input);
                let names = content.parse_terminated(Ident::parse_any, Token![,])?;
                for name in names {
                    if self.skips.contains(&name) {
                        return Err(syn::Error::new(
                            name.span(),
                            "tried to skip the same field twice",
                        ));
                    }
                    self.skips.push(name);
                }
                Ok(())
            }
            "skip_all" => {
                if self.skip_all {
                    return Err(duplicate(ident));
                }
                if !self.skips.is_empty() {
                    return Err(syn::Error::new(
                        ident.span(),
                        "expected either `skip` or `skip_all` argument",
                    ));
                }
                self.skip_all = true;
                Ok(())
            }
            "fields" => {
                let content;
                syn::parenthesized!(content in input);
                let (fields, _no_message) = parse_field_list(&content, FieldContext::Instrument)?;
                set_once(&mut self.fields, ident, fields)
            }
            "ret" => set_once(&mut self.ret, ident, EventArgs::parse_optional(input)?),
            "err" => set_once(&mut self.err, ident, EventArgs::parse_optional(input)?),
            "parent" => Err(syn::Error::new(
                ident.span(),
                "`parent` is not supported (no span parents)",
            )),
            "follows_from" => Err(syn::Error::new(
                ident.span(),
                "`follows_from` is not supported (no span links)",
            )),
            _ => Err(syn::Error::new(
                ident.span(),
                format!("unknown `#[instrument]` argument `{ident}`"),
            )),
        }
    }
}

fn set_once<T>(slot: &mut Option<T>, ident: &Ident, value: T) -> syn::Result<()> {
    if slot.is_some() {
        return Err(duplicate(ident));
    }
    *slot = Some(value);
    Ok(())
}

/// One recordable parameter binding.
enum Param {
    /// `self`, `&self`, `&mut self`, `self: Box<Self>`: recorded with Debug.
    Receiver(Token![self]),
    /// An identifier bound by a typed parameter's pattern.
    Binding(Ident),
}

impl Param {
    fn name(&self) -> String {
        match self {
            Param::Receiver(_) => "self".to_owned(),
            Param::Binding(ident) => unraw(ident),
        }
    }

    fn matches(&self, ident: &Ident) -> bool {
        self.name() == unraw(ident)
    }
}

/// Collects the identifiers bound by an irrefutable parameter pattern, as tracing does.
fn pattern_bindings(pat: &Pat, out: &mut Vec<Param>) {
    match pat {
        Pat::Ident(pat) => out.push(Param::Binding(pat.ident.clone())),
        Pat::Reference(pat) => pattern_bindings(&pat.pat, out),
        Pat::Paren(pat) => pattern_bindings(&pat.pat, out),
        Pat::Type(pat) => pattern_bindings(&pat.pat, out),
        Pat::Struct(pat) => {
            for field in &pat.fields {
                pattern_bindings(&field.pat, out);
            }
        }
        Pat::Tuple(pat) => {
            for elem in &pat.elems {
                pattern_bindings(elem, out);
            }
        }
        Pat::TupleStruct(pat) => {
            for elem in &pat.elems {
                pattern_bindings(elem, out);
            }
        }
        // `_`, `..` and anything rustc rejects in parameter position bind nothing.
        _ => {}
    }
}

/// Replaces every `impl Trait` in `ty` with `_`, so it can annotate a `let`.
fn erase_impl_trait(ty: &Type) -> Type {
    let mut ty = ty.clone();
    erase_in_place(&mut ty);
    ty
}

fn erase_in_place(ty: &mut Type) {
    match ty {
        Type::ImplTrait(_) => {
            *ty = Type::Infer(syn::TypeInfer {
                underscore_token: Token![_](ty.span()),
            });
        }
        Type::Reference(inner) => erase_in_place(&mut inner.elem),
        Type::Ptr(inner) => erase_in_place(&mut inner.elem),
        Type::Slice(inner) => erase_in_place(&mut inner.elem),
        Type::Array(inner) => erase_in_place(&mut inner.elem),
        Type::Paren(inner) => erase_in_place(&mut inner.elem),
        Type::Group(inner) => erase_in_place(&mut inner.elem),
        Type::Tuple(tuple) => tuple.elems.iter_mut().for_each(erase_in_place),
        Type::Path(path) => {
            for segment in &mut path.path.segments {
                if let PathArguments::AngleBracketed(args) = &mut segment.arguments {
                    for arg in &mut args.args {
                        if let GenericArgument::Type(inner) = arg {
                            erase_in_place(inner);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Expands `#[instrument(args)]` applied to `item`.
pub(crate) fn expand(args: TokenStream, item: &TokenStream) -> TokenStream {
    let Ok(function) = syn::parse2::<ItemFn>(item.clone()) else {
        let error = syn::Error::new(
            Span::call_site(),
            "`#[instrument]` can only be applied to functions",
        )
        .to_compile_error();
        return quote!(#error #item);
    };
    match syn::parse2::<InstrumentArgs>(args).and_then(|args| rewrite(&function, &args)) {
        Ok(tokens) => tokens,
        Err(error) => {
            let error = error.to_compile_error();
            quote!(#error #function)
        }
    }
}

/// Rewrites the function body; the signature is emitted unchanged.
fn rewrite(function: &ItemFn, args: &InstrumentArgs) -> syn::Result<TokenStream> {
    let private = quote!(::sc_observability_log::__private);
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = function;
    let outer_attrs = attrs
        .iter()
        .filter(|attr| matches!(attr.style, AttrStyle::Outer));
    let inner_attrs = attrs
        .iter()
        .filter(|attr| matches!(attr.style, AttrStyle::Inner(_)));

    let params = collect_params(sig);
    for skip in &args.skips {
        if !params.iter().any(|param| param.matches(skip)) {
            return Err(syn::Error::new(
                skip.span(),
                "attempting to skip non-existent parameter",
            ));
        }
    }

    let fn_name = LitStr::new(&unraw(&sig.ident), sig.ident.span());
    let name = args
        .name
        .as_ref()
        .map_or_else(|| fn_name.to_token_stream(), Clone::clone);
    let target = args
        .target
        .as_ref()
        .map_or_else(|| quote!(::core::module_path!()), Clone::clone);

    let (level, ok_level, error_level) = completion_levels(args, &private);

    let callsite = Ident::new("__SC_CALLSITE", Span::mixed_site());
    let span = Ident::new("__sc_span", Span::mixed_site());
    let out = Ident::new("__sc_out", Span::mixed_site());

    let fields = record_fields(args, &params, &private);
    let fake_return = fake_return_edge(&sig.output);
    let stmts = &block.stmts;
    let user_block = quote_spanned! {block.span()=>
        {
            #fake_return
            #(#stmts)*
        }
    };

    let run = run_body(sig.asyncness.is_some(), &user_block, &span, &out);

    let completion = completion(args, &span, &out, &private);

    Ok(quote! {
        #(#outer_attrs)*
        #vis #sig {
            #(#inner_attrs)*
            static #callsite: #private::Callsite = #private::Callsite::new(
                #target,
                ::core::option::Option::Some(#name),
            );
            let #span = #private::CallSpan::new(
                &#callsite,
                #private::CallLevels {
                    level: #level,
                    ok: #ok_level,
                    error: #error_level,
                },
                #fields,
            );
            #run
            #completion
            #out
        }
    })
}

/// `(level, ok, error)` completion levels, by the precedence in `docs/compatibility.md`.
fn completion_levels(
    args: &InstrumentArgs,
    private: &TokenStream,
) -> (TokenStream, TokenStream, TokenStream) {
    let level = args.level.clone().unwrap_or(LevelArg::Named("Info"));
    let ok_level = args
        .ret
        .as_ref()
        .and_then(|ret| ret.level.clone())
        .unwrap_or_else(|| level.clone());
    // Without `err` the `error` outcome never occurs; `level` keeps field recording lazy.
    let error_level = args.err.as_ref().map_or_else(
        || level.clone(),
        |err| err.level.clone().unwrap_or(LevelArg::Named("Error")),
    );
    (
        level.tokens(private),
        ok_level.tokens(private),
        error_level.tokens(private),
    )
}

/// Runs the user body inside the entered context and binds its value to `out`.
fn run_body(is_async: bool, user_block: &TokenStream, span: &Ident, out: &Ident) -> TokenStream {
    let entered = Ident::new("__sc_entered", Span::mixed_site());
    if is_async {
        let body = Ident::new("__sc_body", Span::mixed_site());
        let cx = Ident::new("__sc_cx", Span::mixed_site());
        // `entered` is dropped before `poll_fn` returns, so it is never held across `.await`.
        quote! {
            let #out = {
                let mut #body = ::core::pin::pin!(async move #user_block);
                ::core::future::poll_fn(|#cx| {
                    let #entered = #span.enter();
                    ::core::future::Future::poll(::core::pin::Pin::as_mut(&mut #body), #cx)
                })
                .await
            };
        }
    } else {
        quote! {
            #[allow(clippy::redundant_closure_call, clippy::let_unit_value)]
            let #out = {
                let #entered = #span.enter();
                (move || #user_block)()
            };
        }
    }
}

/// Parameter bindings in signature order.
fn collect_params(sig: &syn::Signature) -> Vec<Param> {
    let mut params = Vec::new();
    for input in &sig.inputs {
        match input {
            FnArg::Receiver(receiver) => params.push(Param::Receiver(receiver.self_token)),
            FnArg::Typed(typed) => pattern_bindings(&typed.pat, &mut params),
        }
    }
    params
}

/// The `FnOnce() -> Map` passed to `CallSpan::new`: recorded args, then `fields(..)`.
fn record_fields(args: &InstrumentArgs, params: &[Param], private: &TokenStream) -> TokenStream {
    let custom = args.fields.as_deref().unwrap_or_default();
    // A parameter named like a single-segment custom field is recorded by that field, as in tracing.
    let overridden = |param: &Param| {
        custom.iter().any(|field| match &field.key {
            FieldKey::Static { key, .. } => *key == param.name(),
            FieldKey::Dynamic(_) => false,
        })
    };
    let recorded: Vec<&Param> = params
        .iter()
        .filter(|param| !args.skip_all && !args.skips.iter().any(|skip| param.matches(skip)))
        .filter(|param| !overridden(param))
        .collect();
    if recorded.is_empty() && custom.is_empty() {
        return quote!(#private::Map::new);
    }

    let fields_ident = Ident::new("__sc_fields", Span::mixed_site());
    let mut stmts = Vec::new();
    let mut has_bare = false;
    for param in recorded {
        match param {
            Param::Receiver(self_token) => stmts.push(quote! {
                #private::record_field(
                    &mut #fields_ident,
                    "self",
                    #private::DebugKind.record(&#self_token),
                );
            }),
            Param::Binding(ident) => {
                has_bare = true;
                let field = Field {
                    key: FieldKey::Static {
                        key: unraw(ident),
                        span: ident.span(),
                    },
                    value: FieldValue::Bare(ident_expr(ident)),
                };
                stmts.push(expand_field(&field, stmts.len(), &fields_ident, private));
            }
        }
    }
    for field in custom {
        has_bare |= matches!(field.value, FieldValue::Bare(_));
        stmts.push(expand_field(field, stmts.len(), &fields_ident, private));
    }
    let kind_imports = has_bare.then(|| {
        quote! {
            #[allow(unused_imports)]
            use #private::{DebugKindTag as _, SerializeKindTag as _};
        }
    });
    quote! {
        || {
            #kind_imports
            let mut #fields_ident = #private::Map::new();
            #(#stmts)*
            #fields_ident
        }
    }
}

/// `if false { let x: Ret = loop {}; return x; }`: pins the body's return type for `?` inference.
fn fake_return_edge(output: &ReturnType) -> TokenStream {
    let (ty, span) = match output {
        ReturnType::Default => (
            Type::Tuple(syn::TypeTuple {
                paren_token: syn::token::Paren::default(),
                elems: syn::punctuated::Punctuated::new(),
            }),
            Span::call_site(),
        ),
        ReturnType::Type(_, ty) if matches!(**ty, Type::Never(_)) => return TokenStream::new(),
        ReturnType::Type(_, ty) => (erase_impl_trait(ty), ty.span()),
    };
    let fake = Ident::new("__sc_fake_return", Span::mixed_site());
    quote_spanned! {span=>
        #[allow(
            unknown_lints,
            unreachable_code,
            clippy::diverging_sub_expression,
            clippy::empty_loop,
            clippy::let_unit_value,
            clippy::let_with_type_underscore,
            clippy::needless_return
        )]
        if false {
            let #fake: #ty = loop {};
            return #fake;
        }
    }
}

/// The match that calls `finish_ok` / `finish_err` with the formatted `ret` / `err` value.
fn completion(
    args: &InstrumentArgs,
    span: &Ident,
    out: &Ident,
    private: &TokenStream,
) -> TokenStream {
    let record = Ident::new("__sc_record", Span::mixed_site());
    let value = Ident::new("__sc_value", Span::mixed_site());
    let ret_format = |mode: FormatMode| match mode {
        FormatMode::Display => quote!(#private::display_value),
        FormatMode::Default | FormatMode::Debug => quote!(#private::debug_value),
    };
    let ok_with = |value_expr: TokenStream, mode: FormatMode| {
        let format = ret_format(mode);
        quote! {
            let #record = if #span.enabled(#private::CallOutcome::Ok) {
                ::core::option::Option::Some(#format(#value_expr))
            } else {
                ::core::option::Option::None
            };
            #span.finish_ok(#record);
        }
    };
    match (&args.err, &args.ret) {
        (Some(err), ret) => {
            let ok_arm = ret.as_ref().map_or_else(
                || quote!(#span.finish_ok(::core::option::Option::None);),
                |ret| ok_with(quote!(#value), ret.mode),
            );
            let err_format = match err.mode {
                FormatMode::Debug => quote!(#private::debug_value),
                FormatMode::Default | FormatMode::Display => quote!(#private::display_value),
            };
            let ok_pattern = if ret.is_some() {
                quote!(::core::result::Result::Ok(#value))
            } else {
                quote!(::core::result::Result::Ok(_))
            };
            quote! {
                match &#out {
                    #ok_pattern => {
                        #ok_arm
                    }
                    ::core::result::Result::Err(#value) => {
                        let #record = if #span.enabled(#private::CallOutcome::Error) {
                            #err_format(#value)
                        } else {
                            #private::FieldRecord::Value(#private::Value::Null)
                        };
                        #span.finish_err(#record);
                    }
                }
            }
        }
        (None, Some(ret)) => ok_with(quote!(&#out), ret.mode),
        (None, None) => quote!(#span.finish_ok(::core::option::Option::None);),
    }
}
