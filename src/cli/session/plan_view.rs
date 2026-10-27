//! A switch as the typed plan and step labels the screens show.

use crate::app::apply::Switch;
use crate::app::remove::Plan;
use crate::app::review::{Group, PlanView};
use skillz_harness::action::Action;
use skillz_harness::model::Scope;

pub(super) fn plan_view(switch: &Switch, scope: Scope) -> PlanView {
    let kept = switch.present.iter().map(ToString::to_string).collect();
    let remove = switch
        .removals
        .iter()
        .map(|item| match switch.is_unsourced(item) {
            true => format!("{}  (no known source)", item.label(true)),
            false => item.label(true),
        })
        .collect();
    let install = switch.installs.iter().map(|(e, _)| e.to_string()).collect();
    let update = switch
        .updates
        .iter()
        .map(|(item, _)| item.label(true))
        .collect();
    let skip = switch
        .unsupported
        .iter()
        .map(|u| match &u.reason {
            Some(reason) => format!("{}: {reason}", u.entry),
            None => format!(
                "{}: not available for {} at {scope} scope",
                u.entry, u.agent
            ),
        })
        .collect();
    let groups = [
        (Group::Keep, kept),
        (Group::Remove, remove),
        (Group::Install, install),
        (Group::Update, update),
        (Group::Skip, skip),
    ];
    PlanView {
        targets: None,
        heading: format!("{scope}: {}", switch.to),
        groups: groups
            .into_iter()
            .filter(|(_, lines): &(Group, Vec<String>)| !lines.is_empty())
            .collect(),
        steps: switch.steps().iter().map(Action::describe).collect(),
    }
}

/// What `step` is for and what it does: `removing orca: rm ~/.claude/skills/orca`.
pub(super) fn step_label(switch: &Switch, removal: &Plan, step: &Action) -> String {
    format!("{}: {}", purpose(switch, removal, step), step.describe())
}

/// Installing an entry, updating or removing an installed item.
fn purpose(switch: &Switch, removal: &Plan, step: &Action) -> String {
    let updating = switch
        .updates
        .iter()
        .find(|(_, steps)| steps.contains(step));
    if let Some((item, _)) = updating {
        return format!("updating {}", item.name);
    }
    let installing = switch
        .installs
        .iter()
        .find(|(_, steps)| steps.contains(step));
    match installing {
        Some((entry, _)) => format!("installing {entry}"),
        None => format!("removing {}", removal.owner(step)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::Spec;
    use skillz_harness::model::{Agent, Installed, Installer, Kind};

    fn skill(name: &str) -> Spec {
        Spec::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![Agent::Claude],
        }
    }

    fn removable(name: &str) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
            id: name.into(),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn switch() -> Switch {
        Switch {
            to: "new".into(),
            present: vec![],
            removals: vec![removable("a")],
            unsourced: vec![],
            installs: vec![(skill("b"), vec![])],
            unsupported: vec![],
            updates: vec![],
        }
    }

    #[test]
    fn only_groups_with_lines_are_listed_in_display_order() {
        let view = plan_view(&switch(), Scope::Project);

        let groups: Vec<Group> = view.groups.iter().map(|(g, _)| *g).collect();

        // Nothing is kept or skipped, so only remove and install appear, in that order.
        assert_eq!(groups, [Group::Remove, Group::Install]);
        assert!(view.destructive());
    }
}
