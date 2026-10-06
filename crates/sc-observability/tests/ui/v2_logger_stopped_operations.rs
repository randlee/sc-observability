use sc_observability::{LogEvent, LogQuery, Stopped};

// Keep rustc's diagnostic path stable with and without the legacy v1 `Logger`.
#[allow(dead_code)]
struct Logger<T>(std::marker::PhantomData<T>);

fn stopped_logger_has_no_running_operations(
    logger: sc_observability::v2::Logger<Stopped>,
    event: LogEvent,
    query: LogQuery,
) {
    let _ = logger.log(event);
    let _ = logger.flush();
    let _ = logger.query(&query);
    let _ = logger.shutdown();
}

fn main() {}
