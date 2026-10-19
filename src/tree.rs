use crate::model::Augmentation;

/// A display tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub label: String,
    pub children: Vec<Node>,
}

/// Group augmentations for display: group -> item -> item.children. Sorted, deterministic.
pub fn build(items: &[Augmentation]) -> Vec<Node> {
    let _ = items;
    todo!("lane: tui")
}

/// Render as a box-drawing text tree (`├──`, `└──`, `│`).
pub fn render(nodes: &[Node]) -> String {
    let _ = nodes;
    todo!("lane: tui")
}
