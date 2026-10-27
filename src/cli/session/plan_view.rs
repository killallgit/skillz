//! A switch as the typed plan and step labels the screens show.

use crate::app::apply::Switch;
use crate::app::remove::Plan;
use crate::app::review::{Group, Listed, PlanView};
use skillz_harness::action::Action;
use skillz_harness::model::{Installed, Scope, split_plugin_id};
use skillz_harness::redact::redact;

pub(super) fn plan_view(switch: &Switch, scope: Scope) -> PlanView {
    let kept = switch.present.iter().map(ToString::to_string).collect();
    let remove = switch
        .removals
        .iter()
        .map(|item| match switch.is_unsourced(item) {
            true => format!("{}  (no known source)", plan_label(item)),
            false => plan_label(item),
        })
        .collect();
    let install = switch.installs.iter().map(|(e, _)| e.to_string()).collect();
    let update = switch
        .updates
        .iter()
        .map(|(item, _)| plan_label(item))
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

/// What a plan installs and removes, one entry per item, for the user to check or leave out.
pub(super) fn item_lines(switch: &Switch) -> Vec<Listed> {
    let removing = switch.removals.iter().map(|item| Listed {
        group: Group::Remove,
        id: item.id.clone(),
        copies: item.parts().iter().map(|part| part.id.clone()).collect(),
        label: plan_label(item),
    });
    let installing = switch.installs.iter().map(|(entry, _)| {
        let text = entry.to_string();
        Listed {
            group: Group::Install,
            id: text.clone(),
            copies: vec![],
            label: text,
        }
    });
    removing.chain(installing).collect()
}

/// An installed item as a plan line: its name, where it comes from when that says something more
/// than the name, and its kind. The agents and the scope are left out: the plan's heading names the
/// scope and its agent boxes name the agents.
fn plan_label(item: &Installed) -> String {
    let (name, marketplace) = split_plugin_id(&item.name);
    let source = item
        .source
        .as_deref()
        .filter(|_| marketplace.is_empty())
        .unwrap_or(marketplace);
    let from = match source {
        s if s.is_empty() || s == name => String::new(),
        s => format!(" [{}]", redact(s)),
    };
    format!("{name}{from}  ({})", item.kind)
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

    fn plugin(name: &str, group: &str) -> Installed {
        Installed {
            kind: Kind::Plugin,
            group: group.into(),
            source: None,
            agents: vec![Agent::Claude, Agent::Codex],
            ..removable(name)
        }
    }

    #[test]
    fn a_plugin_named_for_its_marketplace_says_so_once_and_leaves_out_agents_and_scope() {
        let same = plan_label(&plugin("killall-skills@killall-skills", "killall-skills"));
        let other = plan_label(&plugin("tdd@killall-skills", "killall-skills"));

        assert_eq!(same, "killall-skills  (plugin)");
        assert_eq!(other, "tdd [killall-skills]  (plugin)");
    }

    #[test]
    fn a_skill_names_its_source_unless_that_is_its_name() {
        let from_repo = Installed {
            source: Some("o/r".into()),
            ..removable("tdd")
        };
        let local = removable("tdd");

        assert_eq!(plan_label(&from_repo), "tdd [o/r]  (skill)");
        assert_eq!(plan_label(&local), "tdd  (skill)");
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
