use crate::tree::Node;

/// Run the interactive collapsible tree browser until the user quits.
pub fn run(title: &str, nodes: &[Node]) -> anyhow::Result<()> {
    let _ = (title, nodes);
    todo!("lane: tui")
}
