//! The terminal, owned once: entered on the first screen, kept while a `Hold` is out, and
//! always given back.

use std::cell::{Cell, RefCell};
use std::io::IsTerminal;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use super::app::{App, Exit};
use super::hygiene::{self, Signals};
use super::theme::Theme;
use crate::prompt::Hold;

/// How long to wait for an event before looking at the signal flags again.
const TICK: Duration = Duration::from_millis(200);

pub struct Shell {
    term: RefCell<Option<DefaultTerminal>>,
    signals: RefCell<Option<Signals>>,
    holds: Cell<usize>,
    hooked: Cell<bool>,
    mouse: bool,
}

/// The shell and the theme every screen on it is drawn with.
#[derive(Clone, Copy)]
pub struct Ui<'a> {
    pub shell: &'a Shell,
    pub theme: &'a Theme,
}

impl Ui<'_> {
    /// Run `app` until it yields an act (`Some`) or quits (`None`).
    pub fn run<A>(&self, app: &mut App<A>) -> anyhow::Result<Option<A>> {
        self.shell.run(app, self.theme)
    }
}

impl Shell {
    pub fn new(mouse: bool) -> Self {
        Shell {
            term: RefCell::default(),
            signals: RefCell::default(),
            holds: Cell::new(0),
            hooked: Cell::new(false),
            mouse,
        }
    }

    /// Keep the terminal entered between screens until the `Hold` is dropped.
    pub fn hold(&self) -> Hold<'_> {
        self.holds.set(self.holds.get() + 1);
        Hold::new(move || {
            self.holds.set(self.holds.get() - 1);
            if !self.is_held() {
                self.leave();
            }
        })
    }

    pub fn is_held(&self) -> bool {
        self.holds.get() > 0
    }

    pub fn run<A>(&self, app: &mut App<A>, theme: &Theme) -> anyhow::Result<Option<A>> {
        self.enter()?;
        let result = self.drive(app, theme);
        if !self.is_held() {
            self.leave();
        }
        result
    }

    /// Draw one frame now, entering the terminal if needed, without waiting for an event.
    pub fn paint(&self, draw: impl FnOnce(&mut Frame)) -> anyhow::Result<()> {
        self.enter()?;
        let mut term = self.term.borrow_mut();
        let term = term.as_mut().expect("entered above");
        term.draw(draw)?;
        Ok(())
    }

    /// Hand the entered terminal to a painter on another thread; `take_back` returns it.
    pub fn lend_terminal(&self) -> anyhow::Result<DefaultTerminal> {
        self.enter()?;
        let lent = self.term.borrow_mut().take();
        lent.ok_or_else(|| anyhow::anyhow!("the terminal is already lent"))
    }

    pub fn take_back(&self, terminal: DefaultTerminal) {
        *self.term.borrow_mut() = Some(terminal);
    }

    /// Drop the keys pressed while nothing was reading them, so they answer no later question.
    pub fn discard_input(&self) {
        while event::poll(Duration::ZERO).unwrap_or(false) {
            if event::read().is_err() {
                break;
            }
        }
    }

    /// Give the terminal back now; the next screen takes it again.
    pub fn leave(&self) {
        if self.term.borrow_mut().take().is_some() {
            hygiene::restore();
            self.set_inside(false);
        }
    }

    fn enter(&self) -> anyhow::Result<()> {
        if self.term.borrow().is_some() {
            return Ok(());
        }
        if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
            anyhow::bail!("interactive mode needs a terminal; pass --yes to run non-interactively");
        }
        if !self.hooked.replace(true) {
            hygiene::install_panic_hook();
        }
        if self.signals.borrow().is_none() {
            *self.signals.borrow_mut() = Some(Signals::register()?);
        }
        let terminal = ratatui::try_init().inspect_err(|_| hygiene::restore())?;
        *self.term.borrow_mut() = Some(terminal);
        hygiene::enter_extras(self.mouse).inspect_err(|_| self.leave())?;
        self.set_inside(true);
        Ok(())
    }

    fn drive<A>(&self, app: &mut App<A>, theme: &Theme) -> anyhow::Result<Option<A>> {
        loop {
            self.draw(app, theme)?;
            let Some(event) = self.next_event()? else {
                if self.terminated() {
                    return Ok(None);
                }
                continue;
            };
            match app.dispatch(&event) {
                None => {}
                Some(Exit::Quit) => return Ok(None),
                Some(Exit::Act(act)) => return Ok(Some(act)),
            }
        }
    }

    fn draw<A>(&self, app: &mut App<A>, theme: &Theme) -> anyhow::Result<()> {
        let mut term = self.term.borrow_mut();
        let term = term.as_mut().expect("drawn only while entered");
        term.draw(|frame| app.draw(frame, theme))?;
        Ok(())
    }

    /// The next event worth giving to a screen; `None` when a signal or a suspend interrupted
    /// the wait or a tick went by quietly, and the screen should be drawn again.
    fn next_event(&self) -> anyhow::Result<Option<Event>> {
        loop {
            if self.terminated() {
                return Ok(None);
            }
            if self.signals_say_stop() {
                self.suspend()?;
                return Ok(None);
            }
            if !event::poll(TICK)? {
                return Ok(None);
            }
            match event::read()? {
                event if is_suspend(&event) => {
                    self.suspend()?;
                    return Ok(None);
                }
                Event::Key(key) if key.kind != KeyEventKind::Press => {}
                event @ (Event::Key(_) | Event::Mouse(_) | Event::Resize(..)) => {
                    return Ok(Some(event));
                }
                _ => {}
            }
        }
    }

    fn set_inside(&self, inside: bool) {
        if let Some(signals) = self.signals.borrow().as_ref() {
            signals.set_inside(inside);
        }
    }

    fn terminated(&self) -> bool {
        self.signals
            .borrow()
            .as_ref()
            .is_some_and(Signals::terminated)
    }

    fn signals_say_stop(&self) -> bool {
        self.signals
            .borrow()
            .as_ref()
            .is_some_and(Signals::take_stop)
    }

    fn suspend(&self) -> anyhow::Result<()> {
        self.leave();
        hygiene::stop_process()?;
        self.enter()
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.leave();
    }
}

fn is_suspend(event: &Event) -> bool {
    matches!(event, Event::Key(key)
        if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{ctrl, key};

    #[test]
    fn a_hold_is_held_until_it_is_dropped() {
        let shell = Shell::new(false);

        let hold = shell.hold();
        let while_held = shell.is_held();
        drop(hold);

        assert_eq!((while_held, shell.is_held()), (true, false));
    }

    #[test]
    fn nested_holds_release_with_the_last_one() {
        let shell = Shell::new(false);
        let outer = shell.hold();
        let inner = shell.hold();

        drop(inner);
        let after_inner = shell.is_held();
        drop(outer);

        assert_eq!((after_inner, shell.is_held()), (true, false));
    }

    #[test]
    fn only_ctrl_z_suspends() {
        let suspends = [
            is_suspend(&Event::Key(ctrl('z'))),
            is_suspend(&Event::Key(key(KeyCode::Char('z')))),
            is_suspend(&Event::Key(ctrl('c'))),
        ];

        // Ctrl-Z alone is the suspend key.
        assert_eq!(suspends, [true, false, false]);
    }
}
