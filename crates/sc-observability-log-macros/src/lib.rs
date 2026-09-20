//! Procedural macros for `sc-observability-log`.
//!
//! This crate is an implementation detail of `sc-observability-log`, which
//! depends on it through an exact `=` version pin and re-exports every macro at
//! its crate root. Expansions target the `#[doc(hidden)]
//! sc_observability_log::__private` module, which is outside semver, so the two
//! crates always move in lockstep. Depend on `sc-observability-log` rather than
//! on this crate directly.
//!
//! The event macros [`trace!`], [`debug!`], [`info!`], [`warn!`], [`error!`] and
//! [`event!`] accept the `tracing` 0.1 event syntax, and [`macro@instrument`]
//! accepts the `tracing::instrument` arguments; migrating is an import rename.
//! See `sc-observability-log/docs/compatibility.md` for the supported grammar
//! and the forms rejected at compile time.
//!
//! This crate never rewrites labels: `target:` / `name:` values and `{ KEY }`
//! keys are labelled at runtime by `sc-observability-log`.

mod event;
mod fields;
mod instrument;

use proc_macro::TokenStream;

use event::{ImpliedLevel, LevelSource};
use fields::{EventInput, LeveledInput};

fn leveled(input: TokenStream, level: ImpliedLevel) -> TokenStream {
    match syn::parse::<LeveledInput>(input) {
        Ok(LeveledInput(spec)) => event::expand(&spec, LevelSource::Implied(level)).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Emits a `Trace` event with tracing 0.1 syntax.
///
/// ```ignore
/// use sc_observability_log::trace;
/// trace!(target: "app.sync", attempt = 1, "retrying {}", path);
/// ```
#[proc_macro]
pub fn trace(input: TokenStream) -> TokenStream {
    leveled(input, ImpliedLevel::Trace)
}

/// Emits a `Debug` event with tracing 0.1 syntax.
///
/// ```ignore
/// use sc_observability_log::debug;
/// debug!(?request, "request received");
/// ```
#[proc_macro]
pub fn debug(input: TokenStream) -> TokenStream {
    leveled(input, ImpliedLevel::Debug)
}

/// Emits an `Info` event with tracing 0.1 syntax.
///
/// ```ignore
/// use sc_observability_log::info;
/// info!(name: "sync", target: "btit.sync", project, issues, "synced {} issues", issues);
/// ```
#[proc_macro]
pub fn info(input: TokenStream) -> TokenStream {
    leveled(input, ImpliedLevel::Info)
}

/// Emits a `Warn` event with tracing 0.1 syntax.
///
/// ```ignore
/// use sc_observability_log::warn;
/// warn!({ error = %err, retry = true }, "sync failed");
/// ```
#[proc_macro]
pub fn warn(input: TokenStream) -> TokenStream {
    leveled(input, ImpliedLevel::Warn)
}

/// Emits an `Error` event with tracing 0.1 syntax.
///
/// ```ignore
/// use sc_observability_log::error;
/// error!(error = %err, "write failed");
/// ```
#[proc_macro]
pub fn error(input: TokenStream) -> TokenStream {
    leveled(input, ImpliedLevel::Error)
}

/// Emits an event at the `sc_observability_log::Level` given after the prefixes.
///
/// ```ignore
/// use sc_observability_log::{event, Level};
/// const LVL: Level = Level::WARN;
/// event!(name: "sync", Level::INFO, k = 1, "message");
/// event!(LVL, "message");
/// ```
#[proc_macro]
pub fn event(input: TokenStream) -> TokenStream {
    let spec = match syn::parse::<EventInput>(input) {
        Ok(EventInput(spec)) => spec,
        Err(error) => return error.to_compile_error().into(),
    };
    match spec.level.as_ref() {
        Some(level) => event::expand(&spec, LevelSource::Expression(level)).into(),
        None => syn::Error::new(proc_macro2::Span::call_site(), "expected a level")
            .to_compile_error()
            .into(),
    }
}

/// Instruments a sync or `async` function with `tracing::instrument` arguments.
///
/// Each call emits one completion event carrying `duration_ms` and an outcome
/// (`ok`, `error`, `panicked` or `cancelled`), and every event emitted inside
/// the call carries the call's trace context. See
/// `sc-observability-log/docs/compatibility.md`, section "`#[instrument]`".
///
/// ```ignore
/// use sc_observability_log::instrument;
/// #[instrument(name = "bd_update", target = "btit.issues", skip(payload), err(level = "warn"))]
/// async fn bd_update(id: String, payload: Payload) -> Result<Issue, String> { .. }
/// ```
#[proc_macro_attribute]
pub fn instrument(args: TokenStream, item: TokenStream) -> TokenStream {
    instrument::expand(args.into(), &item.into()).into()
}
