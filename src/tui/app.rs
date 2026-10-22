//! A screen and the result it returns to the terminal loop.

use ratatui::Frame;
use ratatui::crossterm::event::Event;

use super::theme::Theme;

pub enum Step<A> {
    Stay,
    Quit,
    Act(A),
}

pub trait Screen<A> {
    fn handle(&mut self, event: &Event) -> Step<A>;
    fn draw(&mut self, frame: &mut Frame, theme: &Theme);
}

#[derive(Debug, PartialEq, Eq)]
pub enum Exit<A> {
    Quit,
    Act(A),
}

pub struct App<'a, A> {
    screen: Box<dyn Screen<A> + 'a>,
}

impl<'a, A> App<'a, A> {
    pub fn new(screen: impl Screen<A> + 'a) -> Self {
        App {
            screen: Box::new(screen),
        }
    }

    pub fn dispatch(&mut self, event: &Event) -> Option<Exit<A>> {
        match self.screen.handle(event) {
            Step::Stay => None,
            Step::Quit => Some(Exit::Quit),
            Step::Act(act) => Some(Exit::Act(act)),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        self.screen.draw(frame, theme);
    }
}
