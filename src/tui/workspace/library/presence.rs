//! Whether each library item is installed right now, in the project and at user level.

use super::rows::Row;
use crate::model::Augmentation;
use crate::profile::{Entry, same_thing};
use crate::snapshot::snapshot;
use crate::tui::workspace::Model;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Presence {
    pub project: bool,
    pub global: bool,
}

/// One presence per row, in row order; sections are never installed.
pub(super) fn presences(rows: &[Row], model: &Model) -> Vec<Presence> {
    let project = entries_of(&model.installed);
    let global = entries_of(&model.global);
    rows.iter()
        .map(|row| match row {
            Row::Item(item) => Presence {
                project: holds(&project, &item.entry),
                global: holds(&global, &item.entry),
            },
            Row::Section { .. } => Presence::default(),
        })
        .collect()
}

fn entries_of(installed: &[Augmentation]) -> Vec<Entry> {
    snapshot("installed", installed).profile.entries
}

fn holds(installed: &[Entry], entry: &Entry) -> bool {
    installed.iter().any(|have| same_thing(have, entry))
}

#[cfg(test)]
mod tests {
    use super::super::rows::{build, tests::skill};
    use super::*;
    use crate::library::Catalog;
    use crate::model::{Agent, Installer, Kind, Origin, Scope};

    fn installed_tdd(scope: Scope) -> Augmentation {
        Augmentation {
            hooks: vec![],
            id: "skill:tdd".into(),
            name: "tdd".into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope,
            group: "o/r".into(),
            source: Some("o/r".into()),
            installer: Installer::Npx,
            origin: Origin::default(),
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn an_item_is_present_where_it_is_installed_and_sections_nowhere() {
        let rows = build(&Catalog::from_items(vec![skill("o/r", "tdd")]));
        let model = Model {
            global: vec![installed_tdd(Scope::Global)],
            ..Model::default()
        };

        let presence = presences(&rows, &model);

        assert_eq!(presence[0], Presence::default());
        assert_eq!(
            presence[1],
            Presence {
                project: false,
                global: true
            }
        );
    }

    #[test]
    fn an_item_that_is_not_installed_is_present_nowhere() {
        let rows = build(&Catalog::from_items(vec![skill("o/r", "grill")]));
        let model = Model {
            installed: vec![installed_tdd(Scope::Project)],
            ..Model::default()
        };

        assert_eq!(presences(&rows, &model)[1], Presence::default());
    }
}
