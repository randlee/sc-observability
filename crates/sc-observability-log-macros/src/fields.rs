//! Parser for the tracing 0.1 event field grammar.
//!
//! [`EventSpec`] is the single parse result for `trace!` .. `error!` and
//! `event!`: the optional `name:` / `target:` prefixes, the level expression
//! (`event!` only), the field list (bare or inside one brace group) and the
//! format tail. [`parse_field_list`] is shared with `#[instrument(fields(..))]`
//! (sprint a-3) through [`FieldContext`].
//!
//! This module only parses and rejects unsupported forms. It never rewrites a
//! label: `target:`/`name:` expressions and `{ KEY }` keys are passed through
//! unchanged and labelled at runtime by `sc-observability-log`.

use proc_macro2::{Span, TokenStream};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitStr, Token, braced, token};

/// Field keys starting with this prefix are reserved for `sc-observability-log`.
///
/// Must equal `sc_observability_log::__private::RESERVED_FIELD_PREFIX`; a
/// proc-macro crate cannot import it, and the runtime test in
/// `tests/macros_jsonl.rs` covers the runtime side.
const RESERVED_PREFIX: &str = "sc_observability_log.";

/// Where a field list is parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldContext {
    /// Event macros: a value-less field is shorthand (`info!(v)`), and a format
    /// message may follow the fields.
    Event,
    /// `#[instrument(fields(..))]`: a value-less field is a deferred field and is
    /// rejected, and no message is allowed.
    Instrument,
}

/// How the field value is recorded.
pub(crate) enum FieldValue {
    /// `k = v` or shorthand `v`: Serialize JSON, else the Debug string.
    Bare(Expr),
    /// `k = ?v` or `?v`: the Debug string.
    Debug(Expr),
    /// `k = %v` or `%v`: the Display string.
    Display(Expr),
}

impl FieldValue {
    /// The value expression.
    pub(crate) fn expr(&self) -> &Expr {
        match self {
            FieldValue::Bare(expr) | FieldValue::Debug(expr) | FieldValue::Display(expr) => expr,
        }
    }
}

/// The key of one field.
pub(crate) enum FieldKey {
    /// A dotted identifier path or a string literal, already joined, with `r#`
    /// prefixes removed.
    Static {
        /// The key text.
        key: String,
        /// Span of the key tokens.
        span: Span,
    },
    /// `{ KEY } = v`: a constant `&'static str` expression, labelled at runtime.
    ///
    /// Boxed to keep `FieldKey` small: `Expr` is a large enum and would
    /// otherwise dwarf the `Static` variant.
    Dynamic(Box<Expr>),
}

/// One parsed field.
pub(crate) struct Field {
    /// Field key.
    pub(crate) key: FieldKey,
    /// Field value.
    pub(crate) value: FieldValue,
}

/// The whole input of one event macro.
pub(crate) struct EventSpec {
    /// `name:` expression, unchanged.
    pub(crate) name: Option<Expr>,
    /// `target:` expression, unchanged.
    pub(crate) target: Option<Expr>,
    /// The level expression of `event!`; `None` for the level macros.
    pub(crate) level: Option<Expr>,
    /// Fields in call order.
    pub(crate) fields: Vec<Field>,
    /// Format tail (`"fmt {}", a`), passed to `format!` unchanged.
    pub(crate) message: Option<TokenStream>,
}

/// Which macro is being parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventMacro {
    /// `trace!` .. `error!`: the level is implied.
    Leveled,
    /// `event!`: a level expression follows the prefixes.
    Event,
}

impl EventSpec {
    /// Parses one event macro input.
    pub(crate) fn parse(input: ParseStream<'_>, kind: EventMacro) -> syn::Result<EventSpec> {
        let (name, target) = parse_prefixes(input)?;
        let level = match kind {
            EventMacro::Leveled => None,
            EventMacro::Event => {
                let level: Expr = input.parse()?;
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
                Some(level)
            }
        };

        let (fields, message) = if input.peek(token::Brace) && !peek_brace_key(input) {
            let content;
            braced!(content in input);
            let (fields, inner_message) = parse_field_list(&content, FieldContext::Event)?;
            if let Some(message) = inner_message {
                return Err(syn::Error::new(
                    message.span(),
                    "a format message cannot appear inside the field braces",
                ));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
            let message = (!input.is_empty())
                .then(|| input.parse::<TokenStream>())
                .transpose()?;
            (fields, message)
        } else {
            parse_field_list(input, FieldContext::Event)?
        };

        Ok(EventSpec {
            name,
            target,
            level,
            fields,
            message,
        })
    }
}

/// Parses a leveled macro input (`trace!` .. `error!`).
pub(crate) struct LeveledInput(pub(crate) EventSpec);

impl Parse for LeveledInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        EventSpec::parse(input, EventMacro::Leveled).map(LeveledInput)
    }
}

/// Parses an `event!` input.
pub(crate) struct EventInput(pub(crate) EventSpec);

impl Parse for EventInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        EventSpec::parse(input, EventMacro::Event).map(EventInput)
    }
}

/// `true` when the next tokens are `ident :` (a single colon, not `::`).
fn peek_prefix(input: ParseStream<'_>, word: &str) -> bool {
    let fork = input.fork();
    let Ok(ident) = Ident::parse_any(&fork) else {
        return false;
    };
    ident == word && fork.peek(Token![:]) && !fork.peek(Token![::])
}

/// Parses the optional `name:` and `target:` prefixes in tracing's order.
fn parse_prefixes(input: ParseStream<'_>) -> syn::Result<(Option<Expr>, Option<Expr>)> {
    let mut name = None;
    let mut target: Option<Expr> = None;
    loop {
        if peek_prefix(input, "parent") {
            let ident = Ident::parse_any(input)?;
            return Err(syn::Error::new(
                ident.span(),
                "`parent:` is not supported (no span parents)",
            ));
        }
        let is_name = peek_prefix(input, "name");
        if !is_name && !peek_prefix(input, "target") {
            return Ok((name, target));
        }
        let ident = Ident::parse_any(input)?;
        input.parse::<Token![:]>()?;
        if is_name && target.is_some() {
            return Err(syn::Error::new(
                ident.span(),
                "`name:` must come before `target:`",
            ));
        }
        let slot = if is_name { &mut name } else { &mut target };
        if slot.is_some() {
            return Err(syn::Error::new(
                ident.span(),
                format!("duplicate `{ident}:`"),
            ));
        }
        *slot = Some(input.parse::<Expr>()?);
        if input.is_empty() {
            return Ok((name, target));
        }
        input.parse::<Token![,]>()?;
    }
}

/// `true` when the next token is a brace group followed by `=` (a `{ KEY } = v` field).
fn peek_brace_key(input: ParseStream<'_>) -> bool {
    let fork = input.fork();
    let Ok(_content) = (|| -> syn::Result<_> {
        let content;
        braced!(content in fork);
        Ok(content)
    })() else {
        return false;
    };
    fork.peek(Token![=]) && !fork.peek(Token![==])
}

/// Parses a comma-separated field list, stopping at the format tail.
///
/// Returns the fields and, for [`FieldContext::Event`], the format tail when one
/// follows. A string literal not followed by `=`, a macro call, or any token that
/// cannot start a field begins the tail, as in tracing's `valueset!`.
pub(crate) fn parse_field_list(
    input: ParseStream<'_>,
    context: FieldContext,
) -> syn::Result<(Vec<Field>, Option<TokenStream>)> {
    let mut fields = Vec::new();
    while !input.is_empty() {
        if context == FieldContext::Instrument && input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            return Err(syn::Error::new(
                lit.span(),
                "string-literal field keys are not supported in `#[instrument(fields(..))]`; use an identifier",
            ));
        }
        if !starts_field(input) {
            if context == FieldContext::Instrument {
                return Err(input.error("expected a field"));
            }
            return Ok((fields, Some(input.parse::<TokenStream>()?)));
        }
        fields.push(parse_field(input, context)?);
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok((fields, None))
}

/// Classifies the next tokens: `true` when they start a field rather than the format tail.
fn starts_field(input: ParseStream<'_>) -> bool {
    if input.peek(Token![?]) || input.peek(Token![%]) {
        return true;
    }
    if input.peek(token::Brace) {
        return peek_brace_key(input);
    }
    if input.peek(LitStr) {
        return input.peek2(Token![=]) && !input.peek2(Token![==]);
    }
    if input.peek(Ident::peek_any) {
        // `concat!(..)` / `format_args!(..)`: a macro call is a message.
        return !input.peek2(Token![!]);
    }
    false
}

/// Parses `a.b.c` (identifiers or keywords, `r#` removed) into the key text.
fn parse_dotted_key(input: ParseStream<'_>) -> syn::Result<(String, Span, Expr)> {
    let first = Ident::parse_any(input)?;
    let span = first.span();
    let mut key = unraw(&first);
    let mut path = TokenStream::new();
    quote::ToTokens::to_tokens(&first, &mut path);
    while input.peek(Token![.]) {
        let dot: Token![.] = input.parse()?;
        let segment = Ident::parse_any(input)?;
        key.push('.');
        key.push_str(&unraw(&segment));
        quote::ToTokens::to_tokens(&dot, &mut path);
        quote::ToTokens::to_tokens(&segment, &mut path);
    }
    let expr = syn::parse2::<Expr>(path)?;
    Ok((key, span, expr))
}

pub(crate) fn unraw(ident: &Ident) -> String {
    let text = ident.to_string();
    match text.strip_prefix("r#") {
        Some(stripped) => stripped.to_owned(),
        None => text,
    }
}

/// Rejects empty and reserved static keys; the error spans all of `tokens`.
fn check_static_key(key: &str, tokens: &dyn quote::ToTokens) -> syn::Result<()> {
    if key.is_empty() {
        return Err(syn::Error::new_spanned(
            tokens,
            "empty field key is not supported",
        ));
    }
    if key.starts_with(RESERVED_PREFIX) {
        return Err(syn::Error::new_spanned(
            tokens,
            "field keys starting with `sc_observability_log.` are reserved",
        ));
    }
    Ok(())
}

/// Parses one field.
fn parse_field(input: ParseStream<'_>, context: FieldContext) -> syn::Result<Field> {
    // `?a.b` / `%a.b` shorthand.
    if input.peek(Token![?]) || input.peek(Token![%]) {
        let debug = input.peek(Token![?]);
        if debug {
            input.parse::<Token![?]>()?;
        } else {
            input.parse::<Token![%]>()?;
        }
        let (key, span, expr) = parse_dotted_key(input)?;
        check_static_key(&key, &expr)?;
        let value = if debug {
            FieldValue::Debug(expr)
        } else {
            FieldValue::Display(expr)
        };
        return Ok(Field {
            key: FieldKey::Static { key, span },
            value,
        });
    }

    if input.peek(token::Brace) {
        let braces;
        braced!(braces in input);
        let key: Expr = braces.parse()?;
        if !braces.is_empty() {
            return Err(braces.error("expected a single key expression inside `{ }`"));
        }
        input.parse::<Token![=]>()?;
        let value = parse_value(input)?;
        return Ok(Field {
            key: FieldKey::Dynamic(Box::new(key)),
            value,
        });
    }

    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let key = lit.value();
        check_static_key(&key, &lit)?;
        input.parse::<Token![=]>()?;
        let value = parse_value(input)?;
        return Ok(Field {
            key: FieldKey::Static {
                key,
                span: lit.span(),
            },
            value,
        });
    }

    let (key, span, shorthand) = parse_dotted_key(input)?;
    check_static_key(&key, &shorthand)?;
    if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        let value = parse_value(input)?;
        return Ok(Field {
            key: FieldKey::Static { key, span },
            value,
        });
    }
    if !input.is_empty() && !input.peek(Token![,]) {
        return Err(input.error("expected `=` after field key"));
    }
    match context {
        FieldContext::Event => Ok(Field {
            key: FieldKey::Static { key, span },
            value: FieldValue::Bare(shorthand),
        }),
        FieldContext::Instrument => Err(syn::Error::new_spanned(
            &shorthand,
            format!(
                "deferred field `{key}` without a value is not supported (tracing `field::Empty`)"
            ),
        )),
    }
}

/// Parses a field value: `?expr`, `%expr` or `expr`.
fn parse_value(input: ParseStream<'_>) -> syn::Result<FieldValue> {
    if input.peek(Token![?]) {
        input.parse::<Token![?]>()?;
        return Ok(FieldValue::Debug(input.parse()?));
    }
    if input.peek(Token![%]) {
        input.parse::<Token![%]>()?;
        return Ok(FieldValue::Display(input.parse()?));
    }
    let expr: Expr = input.parse()?;
    if is_field_empty(&expr) {
        return Err(syn::Error::new_spanned(
            &expr,
            "deferred fields (`field::Empty`) are not supported",
        ));
    }
    Ok(FieldValue::Bare(expr))
}

/// `true` for a path ending in `field::Empty`.
fn is_field_empty(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    let mut segments = path.path.segments.iter().rev();
    matches!(
        (segments.next(), segments.next()),
        (Some(last), Some(parent)) if last.ident == "Empty" && parent.ident == "field"
    )
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse::Parser;

    use super::*;

    fn leveled(tokens: TokenStream) -> syn::Result<EventSpec> {
        syn::parse2::<LeveledInput>(tokens).map(|input| input.0)
    }

    fn keys(spec: &EventSpec) -> Vec<String> {
        spec.fields
            .iter()
            .map(|field| match &field.key {
                FieldKey::Static { key, .. } => key.clone(),
                FieldKey::Dynamic(_) => "{dynamic}".to_owned(),
            })
            .collect()
    }

    #[test]
    fn parses_prefixes_fields_and_message() {
        let spec =
            leveled(quote!(name: "n", target: "t", a.b = 1, r#type = ?x, %y, { K } = 2, "m {}", z))
                .unwrap();
        assert!(spec.name.is_some() && spec.target.is_some());
        assert_eq!(keys(&spec), ["a.b", "type", "y", "{dynamic}"]);
        assert_eq!(
            spec.message.unwrap().to_string(),
            quote!("m {}", z).to_string()
        );
    }

    #[test]
    fn parses_brace_field_set() {
        let spec = leveled(quote!(target: "t", { k = 1, ?v, "lit key" = 2 }, "m")).unwrap();
        assert_eq!(keys(&spec), ["k", "v", "lit key"]);
        assert!(spec.message.is_some());
        let spec = leveled(quote!({ K } = 1)).unwrap();
        assert_eq!(keys(&spec), ["{dynamic}"]);
        assert!(spec.message.is_none());
    }

    #[test]
    fn parses_event_level() {
        let spec = syn::parse2::<EventInput>(quote!(name: "n", Level::INFO, { k = 1 }, "m"))
            .unwrap()
            .0;
        assert!(spec.level.is_some());
        assert_eq!(keys(&spec), ["k"]);
    }

    #[test]
    fn rejects_unsupported_forms() {
        for (tokens, message) in [
            (
                quote!(parent: p, "m"),
                "`parent:` is not supported (no span parents)",
            ),
            (
                quote!(target: "t", name: "n", "m"),
                "`name:` must come before `target:`",
            ),
            (
                quote!(k = tracing::field::Empty),
                "deferred fields (`field::Empty`) are not supported",
            ),
            (quote!("" = 1), "empty field key is not supported"),
            (
                quote!({ "sc_observability_log.x" = 1 }),
                "field keys starting with `sc_observability_log.` are reserved",
            ),
            (
                quote!(sc_observability_log.x = 1),
                "field keys starting with `sc_observability_log.` are reserved",
            ),
        ] {
            let Err(err) = leveled(tokens) else {
                panic!("expected {message:?}");
            };
            assert_eq!(err.to_string(), message);
        }
    }

    #[test]
    fn instrument_context_rejects_value_less_fields() {
        let parser = |input: ParseStream<'_>| parse_field_list(input, FieldContext::Instrument);
        assert!(
            parser
                .parse2(quote!(a.b = 1, c = ?d, %e, { K } = 1))
                .is_ok()
        );
        for (tokens, message) in [
            (
                quote!(x),
                "deferred field `x` without a value is not supported (tracing `field::Empty`)",
            ),
            (
                quote!(k = 1, a.b),
                "deferred field `a.b` without a value is not supported (tracing `field::Empty`)",
            ),
            (
                quote!("k" = 1),
                "string-literal field keys are not supported in `#[instrument(fields(..))]`; use an identifier",
            ),
            (
                quote!(x = tracing::field::Empty),
                "deferred fields (`field::Empty`) are not supported",
            ),
        ] {
            let Err(err) = parser.parse2(tokens) else {
                panic!("expected {message:?}");
            };
            assert_eq!(err.to_string(), message);
        }
    }
}
