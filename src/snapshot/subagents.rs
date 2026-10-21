use std::collections::BTreeMap;

use super::skills::sorted;
use super::{Accept, Collector, own};
use crate::model::{Agent, Augmentation, Kind};
use crate::profile::Entry;

/// Subagents copied from a source, or hand-made ones by their directory, saved by file stem.
#[derive(Default)]
pub(super) struct Subagents(BTreeMap<(String, Vec<Agent>), Vec<String>>);

impl Collector for Subagents {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        match (
            item.kind,
            item.source.as_ref().or(item.origin.link.as_ref()),
        ) {
            (Kind::Subagent, Some(source)) => {
                self.0
                    .entry((source.clone(), agents))
                    .or_default()
                    .push(file_stem(item));
                Accept::Taken
            }
            _ => Accept::Pass,
        }
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((source, agents), names)| Entry::Subagent {
                source,
                names: sorted(names),
                agents: own(agents, default),
            })
            .collect()
    }
}

/// The file name is what a copy finds the definition by.
fn file_stem(item: &Augmentation) -> String {
    let file = item.children.iter().find_map(|c| c.strip_prefix("file: "));
    let stem = file.and_then(|f| f.rsplit_once('.').map(|(s, _)| s));
    stem.unwrap_or(&item.name).to_string()
}

#[cfg(test)]
mod tests {
    use super::super::snapshot;
    use super::super::tests::item;
    use super::*;
    use crate::model::{Installer, Origin};

    fn hand_made(file: &str, held_in: &str) -> Augmentation {
        Augmentation {
            origin: Origin {
                link: Some(held_in.into()),
                version: None,
            },
            children: vec![format!("file: {file}")],
            ..item(
                "Reviewer",
                Kind::Subagent,
                Installer::Local,
                &[Agent::Claude],
            )
        }
    }

    #[test]
    fn a_hand_made_subagent_is_kept_by_the_dir_holding_it() {
        let snap = snapshot("p", &[hand_made("rev.md", "/p/.claude/agents")]);

        assert!(snap.skipped.is_empty());
        assert_eq!(
            snap.profile.entries,
            [Entry::Subagent {
                source: "/p/.claude/agents".into(),
                names: vec!["rev".into()],
                agents: vec![Agent::Claude],
            }]
        );
    }

    #[test]
    fn a_copied_subagent_still_comes_from_its_recorded_source() {
        let copied = Augmentation {
            source: Some("o/r".into()),
            ..hand_made("rev.md", "/p/.claude/agents")
        };

        let snap = snapshot("p", &[copied]);

        let Entry::Subagent { source, .. } = &snap.profile.entries[0] else {
            panic!("expected a subagent entry");
        };
        assert_eq!(source, "o/r");
    }
}
