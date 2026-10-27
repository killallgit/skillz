//! Which tools an entry is installed for, shared by every screen that installs library entries.

use skillz_harness::Spec;
use skillz_harness::model::Agent;

/// `entry` for the chosen tools. Several entries at once keep to the tools each names, so an
/// entry for one tool is not widened to the others; an entry left with no tool is dropped.
pub(super) fn for_tools(entry: Spec, chosen: &[Agent], keep_named: bool) -> Option<Spec> {
    if !keep_named || entry.agents().is_empty() {
        return Some(entry.with_agents(chosen.to_vec()));
    }
    let tools: Vec<Agent> = entry
        .agents()
        .iter()
        .copied()
        .filter(|agent| chosen.contains(agent))
        .collect();
    (!tools.is_empty()).then(|| entry.with_agents(tools))
}
