//! A switch as the typed plan and step labels the screens show.

use crate::action::Action;
use crate::apply::Switch;
use crate::model::Scope;
use crate::remove::Plan;
use crate::review::{Group, PlanView};

fn name(profile: &Option<String>) -> &str {
    profile.as_deref().unwrap_or("none")
}

pub(super) fn plan_view(switch: &Switch, scope: Scope) -> PlanView {
    let keep = switch.keep.iter().map(|l| l.entry.to_string()).collect();
    let remove = switch.drop.iter().map(|l| l.entry.to_string()).collect();
    let install = switch.installs.iter().map(|(e, _)| e.to_string()).collect();
    let skip = switch
        .unsupported
        .iter()
        .map(|u| {
            format!(
                "{} for {}: not supported at {scope} scope",
                u.entry, u.agent
            )
        })
        .collect();
    let groups = [
        (Group::Keep, keep),
        (Group::Remove, remove),
        (Group::Install, install),
        (Group::Skip, skip),
    ];
    PlanView {
        heading: format!("{scope}: {} -> {}", name(&switch.from), name(&switch.to)),
        groups: groups
            .into_iter()
            .filter(|(_, lines): &(Group, Vec<String>)| !lines.is_empty())
            .collect(),
        steps: switch.steps().iter().map(Action::describe).collect(),
    }
}

/// What `step` is for: installing an entry or removing an installed item.
pub(super) fn step_label(switch: &Switch, removal: &Plan, step: &Action) -> String {
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
    use crate::apply::Loaded;
    use crate::model::Agent;
    use crate::profile::Entry;

    fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![Agent::Claude],
        }
    }

    fn switch() -> Switch {
        Switch {
            from: Some("old".into()),
            to: None,
            keep: vec![],
            drop: vec![Loaded {
                entry: skill("a"),
                items: vec![],
            }],
            removals: vec![],
            installs: vec![(skill("b"), vec![])],
            unsupported: vec![],
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
