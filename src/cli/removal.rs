use std::io::Write;

use anyhow::bail;

use super::args::Filter;
use super::list::display_tree;
use super::scope::Scene;
use crate::model::{Agent, Augmentation, Kind};
use crate::{remove, spinner};

pub(super) struct UninstallRequest<'a> {
    pub query: Option<&'a str>,
    pub yes: bool,
    pub dry_run: bool,
}

pub(super) fn uninstall(
    scene: &Scene,
    request: &UninstallRequest,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let UninstallRequest {
        query,
        yes,
        dry_run,
    } = *request;
    let items = scene.gather();
    let candidates = remove::candidates(&items, query, yes)?;
    let picked = if dry_run {
        (0..candidates.len()).collect()
    } else {
        let owned: Vec<Augmentation> = candidates.iter().map(|&c| c.clone()).collect();
        scene.deps.prompter.select_items(
            "Select items to remove",
            &display_tree(scene.deps, scene.env, &owned),
        )?
    };
    let chosen: Vec<&Augmentation> = picked
        .into_iter()
        .filter_map(|i| candidates.get(i).copied())
        .collect();
    confirm_and_remove(
        scene,
        &chosen,
        Review {
            dry_run,
            list_items: true,
        },
        out,
    )
}

/// Remove everything the filters leave, except what `keep` names. Hooks belong to other tools
/// (rtk, beads), so they go only when asked for with `--hooks`.
pub(super) fn reset(
    scene: &Scene,
    keep: &[String],
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.gather();
    let chosen = reset_set(&items, scene.filter, keep);
    confirm_and_remove(
        scene,
        &chosen,
        Review {
            dry_run,
            list_items: true,
        },
        out,
    )
}

fn reset_set<'a>(
    items: &'a [Augmentation],
    filter: &Filter,
    keep: &[String],
) -> Vec<&'a Augmentation> {
    let named = |i: &Augmentation, q: &String| {
        [
            Some(i.name.as_str()),
            Some(i.group.as_str()),
            i.source.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|f| f.eq_ignore_ascii_case(q.trim()))
    };
    let spared: Vec<&Augmentation> = items
        .iter()
        .filter(|i| keep.iter().any(|q| named(i, q)))
        .collect();
    // A marketplace takes its plugins with it, so it stays while any of them is spared.
    let spared_groups: Vec<&str> = spared.iter().map(|i| i.group.as_str()).collect();
    let explicit_kinds = !filter.kinds().is_empty();
    items
        .iter()
        .filter(|i| filter.keeps(i))
        .filter(|i| explicit_kinds || !matches!(i.kind, Kind::Hook | Kind::Mcp | Kind::Subagent))
        .filter(|i| !spared.contains(i))
        .filter(|i| !(i.kind == Kind::Marketplace && spared_groups.contains(&i.name.as_str())))
        .collect()
}

/// How a removal is reviewed: a dry run only prints the plan; otherwise it asks first.
#[derive(Clone, Copy)]
pub(super) struct Review {
    pub(super) dry_run: bool,
    pub(super) list_items: bool,
}

pub(super) fn confirm_and_remove(
    scene: &Scene,
    chosen: &[&Augmentation],
    review: Review,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    if chosen.is_empty() {
        writeln!(out, "nothing selected")?;
        return Ok(());
    }
    let plan = remove::Plan::new(chosen, &scene.filter.agents);
    print_plan(out, &plan, review.list_items)?;
    if review.dry_run {
        return Ok(());
    }
    if !scene.deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }
    let outcome = execute_plan(scene, &plan)?;
    report(out, &outcome)
}

fn print_plan(out: &mut dyn Write, plan: &remove::Plan, list_items: bool) -> anyhow::Result<()> {
    if list_items {
        writeln!(out, "Removing:")?;
        for item in &plan.chosen {
            writeln!(out, "  {}", item.label(true))?;
            if let Some((_, others)) = plan.also_removes.iter().find(|(i, _)| i.id == item.id) {
                let others: Vec<String> = others.iter().map(Agent::to_string).collect();
                writeln!(out, "    also removes it for: {}", others.join(", "))?;
            }
        }
    }
    writeln!(out, "Plan:")?;
    for step in &plan.steps {
        writeln!(out, "  {}", step.describe())?;
    }
    Ok(())
}

/// One status line while it works: the thing being removed, not each command's chatter.
fn execute_plan(scene: &Scene, plan: &remove::Plan) -> anyhow::Result<remove::Outcome> {
    let spinner = spinner::Spinner::start("removing");
    let removal = remove::Removal {
        adapters: &scene.deps.adapters,
        env: scene.env,
        scopes: scene.scopes,
        runner: scene.deps.runner,
    };
    let result = remove::run(plan, &removal, &mut |step| {
        if let Some(s) = &spinner {
            s.set(&format!("removing {}", plan.owner(step)));
        }
    });
    drop(spinner);
    result
}

fn report(out: &mut dyn Write, outcome: &remove::Outcome) -> anyhow::Result<()> {
    writeln!(
        out,
        "ran {} commands, removed {} paths, edited {} files",
        outcome.report.ran.len(),
        outcome.report.removed.len(),
        outcome.report.edited.len()
    )?;
    if !outcome.still_installed.is_empty() {
        bail!(
            "still installed after removal: {}",
            outcome.still_installed_names().join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;

    fn item(name: &str, kind: Kind, group: &str) -> Augmentation {
        Augmentation {
            hooks: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: group.into(),
            source: None,
            installer: crate::model::Installer::Local,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn reset_skips_hooks_unless_asked_and_spares_kept_groups_with_their_marketplace() {
        let items = vec![
            item("tdd", Kind::Skill, "local"),
            item("rtk", Kind::Hook, "hooks"),
            item("p@mp", Kind::Plugin, "mp"),
            item("mp", Kind::Marketplace, "mp"),
            item("other@om", Kind::Plugin, "om"),
            item("om", Kind::Marketplace, "om"),
        ];
        let names = |set: Vec<&Augmentation>| -> Vec<String> {
            set.iter().map(|i| i.name.clone()).collect()
        };
        let all = reset_set(&items, &Filter::default(), &[]);
        assert_eq!(names(all), ["tdd", "p@mp", "mp", "other@om", "om"]);
        let hooks = Filter {
            hooks: true,
            ..Filter::default()
        };
        assert_eq!(names(reset_set(&items, &hooks, &[])), ["rtk"]);
        let kept = reset_set(&items, &Filter::default(), &["MP".to_string()]);
        assert_eq!(names(kept), ["tdd", "other@om", "om"]);
        let one = reset_set(&items, &Filter::default(), &["tdd".to_string()]);
        assert_eq!(one.len(), 4);
    }
}
