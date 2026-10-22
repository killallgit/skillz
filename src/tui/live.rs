//! The workspace kept on screen, and redrawn on its own, while a change runs. The work stays on
//! the calling thread; a painter thread borrows the terminal to draw the workspace again every
//! frame, so the spinners turn even while a command blocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ratatui::DefaultTerminal;

use super::shell::Shell;
use super::theme::Theme;
use super::workspace::Shared;
use crate::prompt::Hold;
use crate::review::{ItemEvent, Progress};

const FRAME: Duration = Duration::from_millis(100);

/// The painter thread: it owns the terminal until it is stopped and hands it back.
struct Painter {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<DefaultTerminal>,
}

impl Painter {
    fn start(terminal: DefaultTerminal, screen: Shared, theme: Theme) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = thread::spawn(move || paint_until(&stopped, terminal, &screen, &theme));
        Painter { stop, thread }
    }

    fn finish(self) -> Option<DefaultTerminal> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.join().ok()
    }
}

fn paint_until(
    stop: &AtomicBool,
    mut terminal: DefaultTerminal,
    screen: &Shared,
    theme: &Theme,
) -> DefaultTerminal {
    while !stop.load(Ordering::Relaxed) {
        {
            let mut screen = screen.lock().unwrap_or_else(PoisonError::into_inner);
            screen.tick();
            let _ = terminal.draw(|frame| screen.draw(frame, theme));
        }
        thread::sleep(FRAME);
    }
    terminal
}

/// Progress shown as the workspace itself: each item's row tells where its change stands.
pub struct LiveProgress<'a> {
    shell: &'a Shell,
    screen: Shared,
    painter: Option<Painter>,
    _hold: Hold<'a>,
}

impl<'a> LiveProgress<'a> {
    /// Start painting `screen`; a terminal that cannot be entered leaves the work unpainted.
    pub fn start(
        shell: &'a Shell,
        theme: &Theme,
        screen: Shared,
        (title, planned): (&str, &[ItemEvent]),
    ) -> Self {
        screen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .begin_run(title, planned);
        let hold = shell.hold();
        let painter = shell
            .lend_terminal()
            .ok()
            .map(|terminal| Painter::start(terminal, Arc::clone(&screen), *theme));
        LiveProgress {
            shell,
            screen,
            painter,
            _hold: hold,
        }
    }
}

impl Progress for LiveProgress<'_> {
    fn step(&self, _label: &str) {}

    fn item(&self, event: &ItemEvent) {
        let mut screen = self.screen.lock().unwrap_or_else(PoisonError::into_inner);
        screen.note(event);
    }
}

impl Drop for LiveProgress<'_> {
    fn drop(&mut self) {
        if let Some(terminal) = self.painter.take().and_then(Painter::finish) {
            self.shell.take_back(terminal);
        }
        self.shell.discard_input();
        self.screen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .finish_run();
    }
}
