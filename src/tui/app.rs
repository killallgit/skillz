//! The screen stack: what a screen can ask for and how the stack answers.

use ratatui::Frame;
use ratatui::crossterm::event::Event;

use super::theme::Theme;

/// What a screen wants after an event.
pub enum Step<A> {
    Stay,
    Push(Box<dyn Screen<A>>),
    Pop,
    Quit,
    /// A result for whoever drives the stack; the stack stays as it is.
    Act(A),
}

pub trait Screen<A> {
    fn handle(&mut self, event: &Event) -> Step<A>;
    fn draw(&mut self, frame: &mut Frame, theme: &Theme);
}

/// Why the stack stopped running.
#[derive(Debug, PartialEq, Eq)]
pub enum Exit<A> {
    Quit,
    Act(A),
}

pub struct App<'a, A> {
    stack: Vec<Box<dyn Screen<A> + 'a>>,
}

impl<'a, A> App<'a, A> {
    pub fn new(root: impl Screen<A> + 'a) -> Self {
        App {
            stack: vec![Box::new(root)],
        }
    }

    /// Give the event to the top screen. `Some` when the stack is done for now.
    pub fn dispatch(&mut self, event: &Event) -> Option<Exit<A>> {
        let step = self.stack.last_mut()?.handle(event);
        match step {
            Step::Stay => None,
            Step::Push(screen) => {
                self.stack.push(screen);
                None
            }
            Step::Pop => {
                self.stack.pop();
                self.stack.is_empty().then_some(Exit::Quit)
            }
            Step::Quit => Some(Exit::Quit),
            Step::Act(act) => Some(Exit::Act(act)),
        }
    }

    /// Draw every layer, bottom first, so a screen on top can be a popup over the one below.
    pub fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        for screen in &mut self.stack {
            screen.draw(frame, theme);
        }
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::key;
    use ratatui::crossterm::event::KeyCode;

    /// Answers each event with the next step from its script.
    struct Scripted(Vec<Step<u8>>);

    impl Screen<u8> for Scripted {
        fn handle(&mut self, _: &Event) -> Step<u8> {
            self.0.remove(0)
        }
        fn draw(&mut self, _: &mut Frame, _: &Theme) {}
    }

    fn press() -> Event {
        Event::Key(key(KeyCode::Enter))
    }

    #[test]
    fn push_adds_a_layer_and_pop_returns_to_the_one_below() {
        let child = Scripted(vec![Step::Pop]);
        let mut app = App::new(Scripted(vec![Step::Push(Box::new(child)), Step::Stay]));

        let pushed = app.dispatch(&press());
        let depth = app.depth();
        let popped = app.dispatch(&press());

        // Pushing keeps running and shows two layers; popping keeps running with one.
        assert_eq!((pushed, depth), (None, 2));
        assert_eq!((popped, app.depth()), (None, 1));
    }

    #[test]
    fn popping_the_last_screen_quits() {
        let mut app = App::new(Scripted(vec![Step::Pop]));

        let exit = app.dispatch(&press());

        assert_eq!(exit, Some(Exit::Quit));
    }

    #[test]
    fn an_act_leaves_the_stack_in_place() {
        let mut app = App::new(Scripted(vec![Step::Act(7), Step::Quit]));

        let acted = app.dispatch(&press());
        let depth = app.depth();
        let quit = app.dispatch(&press());

        // The act comes out once, the screen is still there, and Quit then ends it.
        assert_eq!((acted, depth), (Some(Exit::Act(7)), 1));
        assert_eq!(quit, Some(Exit::Quit));
    }
}
