use std::io::Write;

use super::list::{display_tree, now_ms};
use super::removal::{Review, confirm_and_remove};
use super::scope::Scene;
use crate::model::Augmentation;
use crate::{prune, usage};

/// Report findings; duplicates are always in the removal, unused items only when picked.
pub(super) fn prune(scene: &Scene, dry_run: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let items = scene.gather();
    let usage = usage::read(&deps.adapters, env)?;
    let now = now_ms();
    let found = prune::check(&items, &deps.adapters, &usage, now);
    let unused: Vec<&Augmentation> = found
        .never_used
        .iter()
        .copied()
        .chain(found.unused.iter().map(|(i, _)| *i))
        .collect();
    if found.duplicates.is_empty() && unused.is_empty() {
        writeln!(out, "nothing to clean up")?;
        return Ok(());
    }
    for (title, lines) in prune_sections(&found, now) {
        writeln!(out, "{title}:")?;
        for line in lines {
            writeln!(out, "  {line}")?;
        }
    }
    let mut chosen = found.duplicates.clone();
    if dry_run {
        chosen.extend(&unused);
    } else {
        chosen.extend(pick_unused(scene, &unused)?);
    }
    // A dry run already listed every item, with its age, in the sections above.
    let review = Review {
        dry_run,
        list_items: !dry_run,
    };
    confirm_and_remove(scene, &chosen, review, out)
}

/// The non-empty report sections: title and one line per item.
fn prune_sections(found: &prune::Findings, now_ms: u64) -> Vec<(&'static str, Vec<String>)> {
    let days_ago = |ms: u64| now_ms.saturating_sub(ms) / (24 * 60 * 60 * 1000);
    let sections = [
        (
            "Duplicates (also provided by a plugin)",
            found.duplicates.iter().map(|i| i.label(true)).collect(),
        ),
        (
            "Never used",
            found.never_used.iter().map(|i| i.label(true)).collect(),
        ),
        (
            "Unused for 7+ days",
            found
                .unused
                .iter()
                .map(|(i, ms)| format!("{}  last used {}d ago", i.label(true), days_ago(*ms)))
                .collect(),
        ),
    ];
    sections
        .into_iter()
        .filter(|(_, lines): &(_, Vec<String>)| !lines.is_empty())
        .collect()
}

fn pick_unused<'a>(
    scene: &Scene,
    unused: &[&'a Augmentation],
) -> anyhow::Result<Vec<&'a Augmentation>> {
    if unused.is_empty() {
        return Ok(vec![]);
    }
    let owned: Vec<Augmentation> = unused.iter().map(|&i| i.clone()).collect();
    let picked = scene.deps.prompter.select_items(
        "Select unused items to remove",
        &display_tree(scene.deps, scene.env, &owned),
    )?;
    Ok(picked
        .into_iter()
        .filter_map(|i| unused.get(i).copied())
        .collect())
}
