use sc_observability::{Logger, Running, Stopped};

fn stop(logger: Logger<Running>) -> Logger<Stopped> {
    logger.shutdown()
}

fn main() {}
