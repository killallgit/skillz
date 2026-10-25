//! A one-line progress spinner on stderr: `⠋ removing <thing>`, redrawn in place.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Ticks on its own thread so it keeps turning while a child process blocks. Dropping it
/// stops the thread and clears the line.
pub struct Spinner {
    message: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// `None` unless stderr is a terminal, so pipes and logs get no control characters.
    pub fn start(message: &str) -> Option<Spinner> {
        std::io::stderr()
            .is_terminal()
            .then(|| Self::spawn(message))
    }

    fn spawn(message: &str) -> Spinner {
        let message = Arc::new(Mutex::new(message.to_string()));
        let stop = Arc::new(AtomicBool::new(false));
        let (m, s) = (Arc::clone(&message), Arc::clone(&stop));
        let handle = thread::spawn(move || {
            let mut frame = 0;
            while !s.load(Ordering::Relaxed) {
                let text = m.lock().map(|t| t.clone()).unwrap_or_default();
                let mut err = std::io::stderr();
                let _ = write!(err, "\r\x1b[2K{} {text}", FRAMES[frame % FRAMES.len()]);
                let _ = err.flush();
                frame += 1;
                thread::sleep(Duration::from_millis(80));
            }
        });
        Spinner {
            message,
            stop,
            handle: Some(handle),
        }
    }

    pub fn set(&self, message: &str) {
        if let Ok(mut m) = self.message.lock() {
            *m = message.to_string();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let mut err = std::io::stderr();
        let _ = write!(err, "\r\x1b[2K");
        let _ = err.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_changes_the_message_and_drop_stops_the_thread() {
        let s = Spinner::spawn("a");
        s.set("b");
        assert_eq!(*s.message.lock().unwrap(), "b");
        drop(s);
    }

    #[test]
    fn no_spinner_when_stderr_is_not_a_terminal() {
        // `cargo test` captures stderr, so this is the piped case.
        assert!(Spinner::start("x").is_none());
    }
}
