use std::io::Write;

use super::list::display_tree;
use super::removal::confirm_and_remove;
use super::scope::Scene;
use crate::model::Augmentation;
use crate::{prune, usage};

/// Report findings; duplicates are always in the removal, unused items only when picked.
pub(super) fn prune(
    scene: &Scene,
    now_ms: u64,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let items = scene.gather();
    let usage = usage::read(&deps.adapters, env)?;
    let found = prune::check(&items, &deps.adapters, &usage, now_ms);
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
    let days_ago = |ms: u64| now_ms.saturating_sub(ms) / (24 * 60 * 60 * 1000);
    let sections: [(&str, Vec<String>); 3] = [
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
    for (title, lines) in sections.iter().filter(|(_, l)| !l.is_empty()) {
        writeln!(out, "{title}:")?;
        for line in lines {
            writeln!(out, "  {line}")?;
        }
    }
    let mut chosen = found.duplicates.clone();
    if dry_run {
        chosen.extend(&unused);
    } else if !unused.is_empty() {
        let owned: Vec<Augmentation> = unused.iter().map(|&i| i.clone()).collect();
        let picked = deps.prompter.select_items(
            "Select unused items to remove",
            &display_tree(deps, env, &owned),
        )?;
        chosen.extend(picked.into_iter().filter_map(|i| unused.get(i).copied()));
    }
    // A dry run already listed every item, with its age, in the sections above.
    confirm_and_remove(scene, &chosen, dry_run, !dry_run, out)
}
