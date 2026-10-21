//! The interactive prompter: the shared tree for pickers, the workspace screen, a y/n line.

use std::io::Write;

use super::app::{App, Screen};
use super::dialogs::{Ask, Review};
use super::home;
use super::shell::{Shell, Ui};
use super::theme::Theme;
use super::tree_view;
use super::widgets::confirm::ConfirmDialog;
use crate::prompt::{Choice, Hold, Picked, Prompter};
use crate::tree::{self, Node};

/// Terminal prompter. The theme is chosen once by the caller and handed to every screen; the
/// terminal is entered by the first screen and left again once nothing holds it.
pub struct TuiPrompter {
    theme: Theme,
    shell: Shell,
}

impl TuiPrompter {
    pub fn new(theme: Theme, mouse: bool) -> Self {
        TuiPrompter {
            theme,
            shell: Shell::new(mouse),
        }
    }

    fn ui(&self) -> Ui<'_> {
        Ui {
            shell: &self.shell,
            theme: &self.theme,
        }
    }

    /// Run a yes/no screen; a session that ends first answers no.
    fn answer(&self, screen: impl Screen<bool> + 'static) -> anyhow::Result<bool> {
        Ok(self.ui().run(&mut App::new(screen))?.unwrap_or(false))
    }
}

impl Prompter for TuiPrompter {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        tree_view::pick(title, &tree::flat(options), &self.ui())
    }

    fn select_items(&self, title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
        tree_view::pick(title, nodes, &self.ui())
    }

    fn browse(&self, title: &str, groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        tree_view::browse(title, groups, &self.ui())
    }

    fn home(&self, model: home::Model, pos: home::Pos) -> anyhow::Result<(home::Act, home::Pos)> {
        home::run(model, pos, &self.ui())
    }

    fn hold(&self) -> Hold<'_> {
        self.shell.hold()
    }

    fn release_terminal(&self) {
        self.shell.leave();
    }

    fn review(&self, title: &str, body: &str, question: &str) -> anyhow::Result<bool> {
        self.answer(Review::new(title, body, Some(question)))
    }

    fn report(&self, title: &str, body: &str) -> anyhow::Result<()> {
        self.answer(Review::new(title, body, None)).map(|_| ())
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
        if self.shell.is_held() {
            return self.answer(Ask(ConfirmDialog::new("Confirm", question)));
        }
        eprint!("{question} [y/N] ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        Ok(parse_yes(&line))
    }
}

/// True only for `y` / `yes`, case-insensitive, surrounding whitespace ignored.
fn parse_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yes_accepts_only_y_and_yes() {
        let spellings = [
            "y", "Y", "yes", "YES", " Yes\n", "", "\n", "n", "no", "yep", "yess",
        ];

        let verdicts: Vec<bool> = spellings.iter().map(|a| parse_yes(a)).collect();

        // Only the first five spellings are a yes.
        assert_eq!(verdicts[..5], [true; 5]);
        assert!(verdicts[5..].iter().all(|v| !v));
    }
}
