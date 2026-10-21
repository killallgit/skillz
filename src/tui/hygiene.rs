//! Leaving the terminal the way we found it: a panic hook, signal flags, mouse and title.

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;
use signal_hook::consts::{SIGTERM, SIGTSTP};

const TITLE: &str = "skillz";
const PUSH_TITLE: &str = "\x1b[22;0t";
const POP_TITLE: &str = "\x1b[23;0t";

/// Set up the terminal modes ratatui does not: the window title and, when asked, the mouse.
pub fn enter_extras(mouse: bool) -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "{PUSH_TITLE}")?;
    execute!(out, SetTitle(TITLE))?;
    if mouse {
        execute!(out, EnableMouseCapture)?;
    }
    Ok(())
}

/// Undo everything, whatever state it is in. Safe to call twice.
pub fn restore() {
    let mut out = io::stdout();
    let _ = execute!(out, DisableMouseCapture);
    let _ = write!(out, "{POP_TITLE}");
    let _ = out.flush();
    ratatui::restore();
}

/// Restore the terminal before the previous hook prints the panic, so it is readable.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}

/// Flags the signal handlers raise; the event loop looks at them between events. While the
/// terminal is not ours (`inside` is false) the signals keep their default behaviour.
#[derive(Debug, Clone, Default)]
pub struct Signals {
    terminate: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    inside: Arc<AtomicBool>,
}

impl Signals {
    pub fn register() -> io::Result<Self> {
        let signals = Signals::default();
        signals.on(SIGTERM, &signals.terminate)?;
        signals.on(SIGTSTP, &signals.stop)?;
        Ok(signals)
    }

    fn on(&self, signal: i32, flag: &Arc<AtomicBool>) -> io::Result<()> {
        let (flag, inside) = (Arc::clone(flag), Arc::clone(&self.inside));
        // SAFETY: the handler only touches atomics and calls `emulate_default_handler`, which
        // signal-hook documents as safe to call from a signal handler.
        unsafe {
            signal_hook::low_level::register(signal, move || {
                if inside.load(Ordering::Relaxed) {
                    flag.store(true, Ordering::Relaxed);
                } else {
                    let _ = signal_hook::low_level::emulate_default_handler(signal);
                }
            })?;
        }
        Ok(())
    }

    pub fn set_inside(&self, inside: bool) {
        self.inside.store(inside, Ordering::Relaxed);
    }

    /// SIGTERM arrived. Stays set: every screen after it ends at once.
    pub fn terminated(&self) -> bool {
        self.terminate.load(Ordering::Relaxed)
    }

    /// SIGTSTP arrived since the last call.
    pub fn take_stop(&self) -> bool {
        self.stop.swap(false, Ordering::Relaxed)
    }
}

/// Stop the process like a shell's Ctrl-Z does; returns once it is continued.
pub fn stop_process() -> io::Result<()> {
    signal_hook::low_level::raise(signal_hook::consts::SIGSTOP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminate_signal_stays_raised_and_a_stop_is_taken_once() {
        let signals = Signals::default();
        signals.terminate.store(true, Ordering::Relaxed);
        signals.stop.store(true, Ordering::Relaxed);

        let first = (signals.terminated(), signals.take_stop());
        let second = (signals.terminated(), signals.take_stop());

        // Terminate is sticky; the stop request is consumed.
        assert_eq!(first, (true, true));
        assert_eq!(second, (true, false));
    }
}
