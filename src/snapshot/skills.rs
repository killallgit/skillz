use std::collections::BTreeMap;

use super::{Accept, Collector, own};
use crate::model::{Agent, Augmentation, Installer, Kind};
use crate::profile::Entry;

/// Skills `npx skills` fetched from a source, and hand-made ones by the directory holding
/// them, grouped by source and agents.
#[derive(Default)]
pub(super) struct Skills(BTreeMap<(String, Vec<Agent>), Vec<String>>);

impl Collector for Skills {
    fn accept(&mut self, item: &Augmentation, agents: Vec<Agent>) -> Accept {
        let Some(source) = reinstall_source(item) else {
            return Accept::Pass;
        };
        self.0
            .entry((source.clone(), agents))
            .or_default()
            .push(npx_name(item));
        Accept::Taken
    }

    fn finish(&mut self, default: &[Agent]) -> Vec<Entry> {
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|((source, agents), names)| Entry::Skill {
                source,
                names: sorted(names),
                agents: own(agents, default),
            })
            .collect()
    }
}

fn reinstall_source(item: &Augmentation) -> Option<&String> {
    match (item.kind, item.installer) {
        (Kind::Skill, Installer::Npx) => item.source.as_ref(),
        (Kind::Skill, Installer::Local) => item.origin.link.as_ref(),
        _ => None,
    }
}

/// The name npx finds the skill by, which is not always its directory name.
fn npx_name(item: &Augmentation) -> String {
    let renamed = item.plugin_skills().next();
    renamed.unwrap_or(&item.name).to_string()
}

pub(super) fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::super::snapshot;
    use super::super::tests::{both, item};
    use super::*;
    use crate::model::Origin;

    fn hand_made(name: &str, held_in: &str, children: &[&str]) -> Augmentation {
        Augmentation {
            origin: Origin {
                link: Some(held_in.into()),
                version: None,
            },
            children: children.iter().map(|c| c.to_string()).collect(),
            ..item(name, Kind::Skill, Installer::Local, &both())
        }
    }

    #[test]
    fn a_hand_made_skill_is_kept_by_the_dir_holding_it() {
        let skills = [
            hand_made("beads", "/p/.agents/skills", &[]),
            hand_made("tdd", "/p/.agents/skills", &[]),
        ];

        let snap = snapshot("p", &skills);

        assert!(snap.skipped.is_empty());
        assert_eq!(
            snap.profile.entries,
            [Entry::Skill {
                source: "/p/.agents/skills".into(),
                names: vec!["beads".into(), "tdd".into()],
                agents: vec![],
            }]
        );
    }

    #[test]
    fn a_hand_made_skill_is_named_the_way_npx_finds_it() {
        let skills = [hand_made("folder", "/p/skills", &["skill: real"])];

        let snap = snapshot("p", &skills);

        let Entry::Skill { names, .. } = &snap.profile.entries[0] else {
            panic!("expected a skill entry");
        };
        assert_eq!(names, &["real".to_string()]);
    }

    #[test]
    fn a_hand_made_skill_with_no_dir_to_reinstall_from_is_skipped() {
        let skill = item("mine", Kind::Skill, Installer::Local, &both());

        let snap = snapshot("p", &[skill]);

        assert_eq!(snap.skipped.len(), 1);
    }
}
