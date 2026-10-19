use crate::tree::Node;

/// User interaction. Injected so flows are testable without a terminal.
pub trait Prompter {
    /// Let the user pick any subset of `options`; returns chosen indices (ascending).
    /// An empty result means the user cancelled or picked nothing.
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>>;
    fn confirm(&self, question: &str) -> anyhow::Result<bool>;
    /// Browse `groups` (trees from `tree::build`), mark groups or items and pick an action.
    /// `None` when the user quits. Without a tree UI: pick groups, then show them.
    fn browse(&self, title: &str, groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        let labels: Vec<String> = groups.iter().map(|g| g.label.clone()).collect();
        let groups = self.multi_select(title, &labels)?;
        Ok((!groups.is_empty()).then(|| {
            let picked = Picked {
                groups,
                items: vec![],
            };
            (Choice::Show, picked)
        }))
    }
}

/// What to do with the items picked in `Prompter::browse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Show,
    Remove,
}

/// A `browse` selection: whole groups (indices into its `groups`) plus single items
/// (`Node::item` indices) outside them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picked {
    pub groups: Vec<usize>,
    pub items: Vec<usize>,
}

/// Interactive terminal prompter backed by `tui::select` and a y/n line prompt.
pub struct TuiPrompter;

impl Prompter for TuiPrompter {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        crate::tui::select::run(title, options)
    }

    fn browse(&self, title: &str, groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        crate::tui::tree_view::browse(title, groups)
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
        use std::io::Write;
        eprint!("{question} [y/N] ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        Ok(parse_yes(&line))
    }
}

/// True only for `y` / `yes`, case-insensitive, surrounding whitespace ignored.
pub fn parse_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Non-interactive `--yes`: selects every option and confirms everything.
pub struct AssumeYes;

impl Prompter for AssumeYes {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok((0..options.len()).collect())
    }

    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yes_accepts_only_y_and_yes() {
        for yes in ["y", "Y", "yes", "YES", " Yes\n"] {
            assert!(parse_yes(yes), "{yes:?}");
        }
        for no in ["", "\n", "n", "no", "yep", "ye s", "yess"] {
            assert!(!parse_yes(no), "{no:?}");
        }
    }

    #[test]
    fn assume_yes_selects_all_and_confirms() {
        let opts = vec!["a".to_string(), "b".to_string()];
        assert_eq!(AssumeYes.multi_select("t", &opts).unwrap(), vec![0, 1]);
        assert!(AssumeYes.confirm("q").unwrap());
    }
}
