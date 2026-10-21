use std::collections::HashSet;
use std::io::Write;

use super::deps::Deps;
use super::removal::{Review, confirm_and_remove};
use super::scope::Scene;
use crate::cost;
use crate::env::Env;
use crate::model::{Augmentation, Kind};
use crate::prompt::Choice;
use crate::prune;
use crate::tree;
use crate::usage;
use crate::{info, remove};

pub(super) struct ListView<'a> {
    pub(super) query: Option<&'a str>,
    pub(super) all: bool,
    pub(super) json: bool,
    pub(super) interactive: bool,
}

/// Items `list QUERY` shows: the matches, plus the whole group of any matched marketplace.
fn queried(items: &[Augmentation], hits: &[&Augmentation]) -> Vec<Augmentation> {
    let ids: HashSet<String> = hits.iter().map(|i| i.id.clone()).collect();
    let groups: HashSet<String> = hits
        .iter()
        .filter(|i| i.kind == Kind::Marketplace)
        .map(|i| i.group.clone())
        .collect();
    items
        .iter()
        .filter(|i| ids.contains(&i.id) || groups.contains(&i.group))
        .cloned()
        .collect()
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `tree::build` plus each row's info-modal text. Last-use data is optional: if an agent's
/// records cannot be read the modal just omits it.
pub(super) fn display_tree(deps: &Deps, env: &Env, items: &[Augmentation]) -> Vec<tree::Node> {
    let usage = usage::read(&deps.adapters, env).ok();
    let now = now_ms();
    let mut nodes = tree::build(items);
    tree::attach_info(&mut nodes, &|i| {
        let item = &items[i];
        let used = match (&usage, prune::tracked(item, &deps.adapters)) {
            (Some(u), true) => u.last_used(item).map_or(info::Used::Never, info::Used::At),
            _ => info::Used::NotTracked,
        };
        info::item_lines(item, used, now)
    });
    nodes
}

/// Browse `items` in the tree UI, then show or remove whatever the user marks.
fn browse(scene: &Scene, items: &[Augmentation], out: &mut dyn Write) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let nodes = display_tree(deps, env, items);
    let Some((choice, picked)) = deps.prompter.browse("skillz", &nodes)? else {
        return Ok(());
    };
    let chosen: Vec<&Augmentation> = picked
        .item_indices(&nodes)
        .into_iter()
        .filter_map(|i| items.get(i))
        .collect();
    match choice {
        Choice::Show => {
            write!(
                out,
                "{}",
                tree::render(&tree::build(&queried(items, &chosen)))
            )?;
            Ok(())
        }
        Choice::Remove => {
            let review = Review {
                dry_run: false,
                list_items: true,
            };
            confirm_and_remove(scene, &chosen, review, out)
        }
    }
}

pub(super) fn list(scene: &Scene, view: ListView, out: &mut dyn Write) -> anyhow::Result<()> {
    let deps = scene.deps;
    let mut items = scene.gather();
    if let Some(q) = view.query {
        items = queried(&items, &remove::matches(&items, Some(q)));
    }
    if view.json {
        return print_json(scene, &items, out);
    }
    if deps.interactive && (view.interactive || (view.query.is_some() && spans_groups(&items))) {
        return browse(scene, &items, out);
    }
    print_tree(&items, &view, out)
}

/// A query spanning several groups opens the browser so the user can narrow it down.
fn spans_groups(items: &[Augmentation]) -> bool {
    items.iter().any(|i| i.group != items[0].group)
}

/// The items and their removal plans, with each one's context cost.
fn print_json(scene: &Scene, items: &[Augmentation], out: &mut dyn Write) -> anyhow::Result<()> {
    // Read the costs only here: the report takes a second or so (`claude -p /context`).
    let costs = cost::read(&scene.deps.adapters, scene.env, scene.deps.runner);
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(|i| {
            let mut row = serde_json::to_value(i)?;
            row["tokens"] = serde_json::json!(costs.tokens(i));
            anyhow::Ok(row)
        })
        .collect::<Result<_, _>>()?;
    serde_json::to_writer_pretty(&mut *out, &rows)?;
    writeln!(out)?;
    Ok(())
}

fn print_tree(items: &[Augmentation], view: &ListView, out: &mut dyn Write) -> anyhow::Result<()> {
    if items.is_empty() {
        let msg = if view.query.is_some() {
            "nothing installed matches"
        } else {
            "nothing installed"
        };
        writeln!(out, "{msg}")?;
    }
    let nodes = tree::build(items);
    let expand = view.all || view.query.is_some();
    let nodes = if expand { nodes } else { tree::collapse(nodes) };
    write!(out, "{}", tree::render(&nodes))?;
    Ok(())
}
