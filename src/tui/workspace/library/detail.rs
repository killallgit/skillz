//! What the line under the library says about the highlighted item.

use crate::library::{Cue, ItemKind, LibraryItem, Provenance};
use crate::model::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Detail {
    pub kind: ItemKind,
    /// Where it comes from or what it runs.
    pub source: String,
    /// Empty when it goes to whichever agents the profile uses.
    pub agents: Vec<Agent>,
    /// How it was installed; `None` for an item typed in by hand.
    pub provenance: Option<Provenance>,
    pub cue: Cue,
    /// Its local source is gone.
    pub stale: bool,
}

pub(super) fn detail(item: &LibraryItem, stale: bool) -> Detail {
    Detail {
        kind: item.kind(),
        source: item.source(),
        agents: item.entry.agents().to_vec(),
        provenance: item.provenance.clone(),
        cue: Cue::of(item.provenance.as_ref()),
        stale,
    }
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::{hook, skill};
    use super::*;

    #[test]
    fn a_skill_is_described_by_its_source_and_a_hook_by_its_command() {
        let skill = detail(&skill("o/r", "tdd"), false);
        let hook = detail(&hook("notify"), false);

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
    fn an_item_carries_the_provenance_it_was_recorded_with() {
        let provenance = Provenance {
            link: Some("https://h/o/r.git".into()),
            engine: crate::model::Installer::Npx,
            version: None,
            recorded_at: 1,
            detected: false,
        };
        let item = skill("o/r", "tdd").with_provenance(provenance.clone());

        assert_eq!(detail(&item, false).provenance, Some(provenance));
        assert_eq!(detail(&skill("o/r", "tdd"), false).provenance, None);
    }

    #[test]
    fn the_cue_tells_typed_detected_and_skillz_installed_items_apart() {
        let recorded = |detected| {
            skill("o/r", "tdd").with_provenance(Provenance {
                link: None,
                engine: crate::model::Installer::Npx,
                version: None,
                recorded_at: 1,
                detected,
            })
        };

        let cues = [
            detail(&skill("o/r", "tdd"), false).cue,
            detail(&recorded(true), false).cue,
            detail(&recorded(false), false).cue,
        ];

        assert_eq!(cues, [Cue::Manual, Cue::Detected, Cue::Skillz]);
    }

    #[test]
    fn an_item_for_chosen_agents_names_them() {
        let item =
            LibraryItem::from_entry(skill("o/r", "tdd").entry.with_agents(vec![Agent::Codex]));

        assert_eq!(detail(&item, false).agents, [Agent::Codex]);
    }
}
