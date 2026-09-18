// Shared `#[instrument]` compatibility fixture: one instrumented fn per row of
// the argument table in `docs/compatibility.md`. Not a test binary: it is
// `include!`d by `tests/compat_instrument.rs` twice, once after
// `use tracing::{instrument, Level};` (compile-only proof of genuine
// tracing-attributes 0.1.31 syntax) and once after the same import from
// `sc_observability_log` (executed and asserted), and by
// `sc-observability-log-consumer-check` (single-dependency proof). The includer
// supplies the `instrument` and `Level` imports; rejected forms live in `tests/ui/`.

const COMPAT_NAME: &str = "compat.instrument.const_name";
const COMPAT_TARGET: &str = "compat.instrument.const_target";
const COMPAT_LEVEL: Level = Level::WARN;
const COMPAT_KEY: &str = "compat.dynamic";
const COMPAT_DEBUG_KEY: &str = "compat.dynamic_debug";

/// Recorded with `Debug` (no `Serialize`).
#[derive(Debug)]
pub struct CompatWidget {
    id: u32,
}

/// An error with distinct `Debug` and `Display` text.
#[derive(Debug)]
pub struct CompatError {
    code: u32,
}

impl core::fmt::Display for CompatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "compat failure {}", self.code)
    }
}

fn compat_result(fail: bool, value: u32) -> Result<u32, CompatError> {
    if fail {
        Err(CompatError { code: value })
    } else {
        Ok(value)
    }
}

// ---- name ----

#[instrument(name = "compat.name_literal", skip_all)]
fn compat_name_literal() {}

#[instrument(name = COMPAT_NAME, skip_all)]
fn compat_name_const() {}

#[instrument("compat.name_positional", skip_all)]
fn compat_name_positional() {}

#[instrument(skip_all)]
fn compat_default_name() {}

// ---- target ----

#[instrument(name = "compat.target_literal", target = "compat.instrument.literal_target")]
fn compat_target_literal() {}

#[instrument(name = "compat.target_const", target = COMPAT_TARGET)]
fn compat_target_const() {}

// ---- level ----

#[instrument(name = "compat.level_str", level = "WaRn")]
fn compat_level_str() {}

#[instrument(name = "compat.level_path", level = Level::DEBUG)]
fn compat_level_path() {}

#[instrument(name = "compat.level_const", level = COMPAT_LEVEL)]
fn compat_level_const() {}

#[instrument(name = "compat.level_int_trace", level = 1)]
fn compat_level_int_trace() {}

#[instrument(name = "compat.level_int_error", level = 5)]
fn compat_level_int_error() {}

// ---- args: default, skip, skip_all, fields ----

#[instrument(name = "compat.default_args")]
fn compat_default_args(count: u32, label: &str, widget: &CompatWidget) -> u32 {
    count + u32::try_from(label.len()).unwrap_or(0) + widget.id
}

#[instrument(name = "compat.skip_all", skip_all)]
fn compat_skip_all(count: u32, label: &str) -> usize {
    label.len() + usize::try_from(count).unwrap_or(0)
}

#[instrument(
    name = "compat.fields",
    skip_all,
    fields(k = count, ?label, %path, a.b = 1, { COMPAT_KEY } = 1, { COMPAT_DEBUG_KEY } = ?label, r#type = 1)
)]
fn compat_fields(count: u32, label: &str, path: &str) -> usize {
    label.len() + path.len() + usize::try_from(count).unwrap_or(0)
}

impl CompatWidget {
    #[instrument(name = "compat.method_self")]
    fn method_self(&self, n: u32) -> u32 {
        self.id + n
    }

    #[instrument(name = "compat.skip", skip(self, secret))]
    fn method_skip(&mut self, secret: &str, n: u32) {
        self.id += n + u32::try_from(secret.len()).unwrap_or(0);
    }

    #[instrument(name = "compat.method_owned")]
    fn method_owned(self) -> u32 {
        self.id
    }

    /// A `T: Debug` parameter given a `Serialize + Debug` value records its Debug string.
    #[instrument(name = "compat.generic", skip(self))]
    fn generic_debug<T>(&self, t: T) -> String
    where
        T: core::fmt::Debug,
    {
        format!("{}:{t:?}", self.id)
    }
}

// ---- ret ----

/// Outer attributes (this doc comment and `#[inline]`) are kept on the rewritten fn.
#[instrument(name = "compat.ret", ret)]
#[inline]
fn compat_ret(n: u32) -> u32 {
    n * 2
}

#[instrument(name = "compat.ret_debug", ret(Debug), skip_all)]
fn compat_ret_debug() -> &'static str {
    "beads"
}

#[instrument(name = "compat.ret_display", ret(Display), skip_all)]
fn compat_ret_display() -> &'static str {
    "beads"
}

#[instrument(name = "compat.ret_level", ret(level = "debug"))]
fn compat_ret_level(n: u32) -> u32 {
    n
}

#[instrument(name = "compat.ret_debug_level", ret(Debug, level = Level::WARN), skip_all)]
fn compat_ret_debug_level() -> &'static str {
    "beads"
}

#[instrument(name = "compat.ret_display_level", ret(Display, level = 2), skip_all)]
fn compat_ret_display_level() -> &'static str {
    "beads"
}

// ---- err ----

#[instrument(name = "compat.err", err)]
fn compat_err(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 1)
}

#[instrument(name = "compat.err_debug", err(Debug))]
fn compat_err_debug(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 2)
}

#[instrument(name = "compat.err_display", err(Display))]
fn compat_err_display(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 3)
}

#[instrument(name = "compat.err_level", err(level = "warn"))]
fn compat_err_level(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 4)
}

#[instrument(name = "compat.err_debug_level", err(Debug, level = Level::INFO))]
fn compat_err_debug_level(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 5)
}

#[instrument(name = "compat.err_display_level", err(Display, level = COMPAT_LEVEL))]
fn compat_err_display_level(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 6)
}

#[instrument(name = "compat.ret_err", ret, err)]
fn compat_ret_err(fail: bool) -> Result<u32, CompatError> {
    compat_result(fail, 7)
}

// ---- async ----

#[instrument(name = "compat.async_outer", fields(stage = "outer"))]
async fn compat_async_outer(n: u32) -> u32 {
    match compat_async_inner(n).await {
        Ok(value) | Err(CompatError { code: value }) => value + 1,
    }
}

#[instrument(name = "compat.async_inner", ret, err)]
async fn compat_async_inner(n: u32) -> Result<u32, CompatError> {
    let value = core::future::ready(n).await;
    compat_result(false, value)
}

/// An instrumented `async fn` whose body is `Send` still produces a `Send` future.
fn compat_assert_send<F: core::future::Future + Send>(future: F) -> F {
    future
}

/// Polls a future that never stays pending to completion, without a runtime.
fn compat_block_on<F: core::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    loop {
        if let core::task::Poll::Ready(output) =
            core::future::Future::poll(future.as_mut(), &mut cx)
        {
            return output;
        }
        std::thread::yield_now();
    }
}

/// Calls every fixture fn once (`err` fns once with `Ok` and once with `Err`).
///
/// Returns the sum of the returned values, proving the instrumented fns still
/// return what their bodies return.
#[must_use]
pub fn run_compat_instrument() -> usize {
    let mut total: usize = 0;
    let mut add = |value: u32| total += usize::try_from(value).unwrap_or(0);

    compat_name_literal();
    compat_name_const();
    compat_name_positional();
    compat_default_name();
    compat_target_literal();
    compat_target_const();
    compat_level_str();
    compat_level_path();
    compat_level_const();
    compat_level_int_trace();
    compat_level_int_error();

    let widget = CompatWidget { id: 1 };
    add(compat_default_args(3, "beads", &widget));
    add(u32::try_from(compat_skip_all(3, "beads")).unwrap_or(0));
    add(u32::try_from(compat_fields(7, "beads", "a/b")).unwrap_or(0));
    add(widget.method_self(2));
    let mut widget = widget;
    widget.method_skip("secret", 1);
    add(u32::try_from(widget.generic_debug(String::from("serde")).len()).unwrap_or(0));
    add(widget.method_owned());

    add(compat_ret(21));
    add(u32::try_from(compat_ret_debug().len()).unwrap_or(0));
    add(u32::try_from(compat_ret_display().len()).unwrap_or(0));
    add(compat_ret_level(1));
    add(u32::try_from(compat_ret_debug_level().len()).unwrap_or(0));
    add(u32::try_from(compat_ret_display_level().len()).unwrap_or(0));

    let err_fns: [fn(bool) -> Result<u32, CompatError>; 7] = [
        compat_err,
        compat_err_debug,
        compat_err_display,
        compat_err_level,
        compat_err_debug_level,
        compat_err_display_level,
        compat_ret_err,
    ];
    for err_fn in err_fns {
        for fail in [false, true] {
            match err_fn(fail) {
                Ok(value) | Err(CompatError { code: value }) => add(value),
            }
        }
    }

    add(compat_block_on(compat_assert_send(compat_async_outer(40))));
    total
}
