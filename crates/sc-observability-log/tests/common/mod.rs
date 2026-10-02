use std::sync::mpsc;
use std::time::Duration;

pub(crate) struct StdoutHold {
    release: Option<mpsc::Sender<()>>,
    released: mpsc::Receiver<Result<(), String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl StdoutHold {
    pub(crate) fn release(mut self) -> Result<(), String> {
        self.release.take().unwrap().send(()).unwrap();
        let result = self
            .released
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| format!("stdout holder acknowledgement: {error}"))?;
        self.thread.take().unwrap().join().unwrap();
        result
    }
}

impl Drop for StdoutHold {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) fn hold_stdout() -> StdoutHold {
    let (release, receive) = mpsc::channel();
    let (acknowledge, released) = mpsc::channel();
    let (ready, entered) = mpsc::sync_channel(0);
    let holder = std::thread::spawn(move || {
        let stdout = std::io::stdout();
        let lock = stdout.lock();
        ready.send(()).expect("stdout holder ready");
        receive.recv().expect("stdout holder release");
        drop(lock);
        acknowledge
            .send(Ok(()))
            .expect("stdout holder acknowledgement receiver");
    });
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("stdout holder entered");
    StdoutHold {
        release: Some(release),
        released,
        thread: Some(holder),
    }
}
