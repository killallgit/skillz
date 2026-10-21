use crate::tree::Node;
use crate::tui::home::{Act, Model, Pos};

/// User interaction. Injected so flows are testable without a terminal.
pub trait Prompter {
    /// Let the user pick any subset of `options`; returns chosen indices (ascending).
    /// An empty result means the user cancelled or picked nothing.
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>>;
    fn confirm(&self, question: &str) -> anyhow::Result<bool>;
    /// Pick items from `nodes` (trees from `tree::build`); returns the chosen `Node::item`
    /// indices, ascending. Without a tree UI: a flat pick over the item labels.
    fn select_items(&self, title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
        let items = crate::tree::items(nodes);
        let labels: Vec<String> = items.iter().map(|(_, l)| l.clone()).collect();
        Ok(self
            .multi_select(title, &labels)?
            .into_iter()
            .filter_map(|i| items.get(i).map(|(item, _)| *item))
            .collect())
    }
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
    /// The workspace screen: show `model` with the cursors at `pos` until the user asks for
    /// something. Needs a terminal, so the default is an error.
    fn home(&self, _model: Model, _pos: Pos) -> anyhow::Result<(Act, Pos)> {
        anyhow::bail!("the workspace needs a terminal")
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub groups: Vec<usize>,
    pub items: Vec<usize>,
}

impl Picked {
    /// Every `Node::item` this selection covers: all items of the picked groups plus the
    /// single ones, ascending.
    pub fn item_indices(&self, groups: &[Node]) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .groups
            .iter()
            .filter_map(|&g| groups.get(g))
            .flat_map(|g| crate::tree::items(std::slice::from_ref(g)))
            .map(|(i, _)| i)
            .chain(self.items.iter().copied())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
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
    fn assume_yes_selects_all_and_confirms() {
        let opts = vec!["a".to_string(), "b".to_string()];
        assert_eq!(AssumeYes.multi_select("t", &opts).unwrap(), vec![0, 1]);
        assert!(AssumeYes.confirm("q").unwrap());
    }
}
