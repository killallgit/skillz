use std::io::Write;

use anyhow::bail;

use super::args::Filter;
use super::list::display_tree;
use super::scope::Scene;
use crate::model::{Agent, Augmentation, Kind};
use crate::{remove, spinner};

pub(super) fn uninstall(
    scene: &Scene,
    query: Option<&str>,
    yes: bool,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.gather();
    let candidates = remove::candidates(&items, query, yes)?;
    // A dry run changes nothing, so show every candidate's plan without asking.
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
    confirm_and_remove(scene, &chosen, dry_run, true, out)
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
    confirm_and_remove(scene, &chosen, dry_run, true, out)
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

/// Show the removal plan for `chosen` (and the items themselves if `list_items`), confirm,
/// run it, and report what it did.
pub(super) fn confirm_and_remove(
    scene: &Scene,
    chosen: &[&Augmentation],
    dry_run: bool,
    list_items: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let deps = scene.deps;
    if chosen.is_empty() {
        writeln!(out, "nothing selected")?;
        return Ok(());
    }

    let plan = remove::Plan::new(chosen, &scene.filter.agents);
    if list_items {
        writeln!(out, "Removing:")?;
        for c in chosen {
            writeln!(out, "  {}", c.label(true))?;
            if let Some((_, others)) = plan.also_removes.iter().find(|(i, _)| i.id == c.id) {
                let others: Vec<String> = others.iter().map(Agent::to_string).collect();
                writeln!(out, "    also removes it for: {}", others.join(", "))?;
            }
        }
    }
    writeln!(out, "Plan:")?;
    for step in &plan.steps {
        writeln!(out, "  {}", step.describe())?;
    }
    if dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }

    // One status line while it works: the thing being removed, not each command's chatter.
    let spinner = spinner::Spinner::start("removing");
    let result = remove::run(
        &plan,
        &deps.adapters,
        scene.env,
        scene.scopes,
        deps.runner,
        &mut |step| {
            if let Some(s) = &spinner {
                s.set(&format!("removing {}", plan.owner(step)));
            }
        },
    );
    drop(spinner);
    let outcome = result?;
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
            outcome.still_installed.join(", ")
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
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: group.into(),
            source: None,
            installer: crate::model::Installer::Local,
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
