//! What the line under the library says about the highlighted item.

use crate::library::{ItemKind, LibraryItem};
use crate::model::Agent;
use crate::profile::Entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Detail {
    pub kind: ItemKind,
    /// Where it comes from or what it runs.
    pub source: String,
    /// Empty when it goes to whichever agents the profile uses.
    pub agents: Vec<Agent>,
}

pub(super) fn detail(item: &LibraryItem) -> Detail {
    let source = match &item.entry {
        Entry::Skill { source, .. }
        | Entry::Subagent { source, .. }
        | Entry::Plugin { source, .. } => source.clone(),
        Entry::Hook { command, .. } => command.clone(),
        Entry::Mcp { command, url, .. } => command.clone().or(url.clone()).unwrap_or_default(),
    };
    Detail {
        kind: item.kind(),
        source,
        agents: item.entry.agents().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::{hook, skill};
    use super::*;

    #[test]
    fn a_skill_is_described_by_its_source_and_a_hook_by_its_command() {
        let skill = detail(&skill("o/r", "tdd"));
        let hook = detail(&hook("notify"));

        assert_eq!(
            (skill.kind, skill.source.as_str()),
            (ItemKind::Skill, "o/r")
        );
        assert_eq!(
            (hook.kind, hook.source.as_str()),
            (ItemKind::Hook, "notify")
        );
    }

    #[test]
    fn an_item_for_chosen_agents_names_them() {
        let item =
            LibraryItem::from_entry(skill("o/r", "tdd").entry.with_agents(vec![Agent::Codex]));

        assert_eq!(detail(&item).agents, [Agent::Codex]);
    }
}
