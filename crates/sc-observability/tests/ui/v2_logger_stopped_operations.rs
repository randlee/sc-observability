use sc_observability::{LogEvent, LogQuery, Stopped};
use sc_observability::v2::Logger;

fn stopped_logger_has_no_running_operations(
    logger: Logger<Stopped>,
    event: LogEvent,
    query: LogQuery,
) {
    let _ = logger.log(event);
    let _ = logger.flush();
    let _ = logger.query(&query);
    let _ = logger.shutdown();
}

fn main() {}
