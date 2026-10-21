//! The interactive prompter: the shared tree for pickers, the workspace screen, a y/n line.

use std::io::Write;

use super::home;
use super::theme::Theme;
use super::tree_view;
use crate::prompt::{Choice, Picked, Prompter};
use crate::tree::{self, Node};

/// Terminal prompter. The theme is chosen once by the caller and handed to every screen.
pub struct TuiPrompter {
    theme: Theme,
}

impl TuiPrompter {
    pub fn new(theme: Theme) -> Self {
        TuiPrompter { theme }
    }
}

impl Prompter for TuiPrompter {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        tree_view::pick(title, &tree::flat(options), &self.theme)
    }

    fn select_items(&self, title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
        tree_view::pick(title, nodes, &self.theme)
    }

    fn browse(&self, title: &str, groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        tree_view::browse(title, groups, &self.theme)
    }

    fn home(&self, model: home::Model, pos: home::Pos) -> anyhow::Result<(home::Act, home::Pos)> {
        home::run(model, pos, &self.theme)
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
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
