//! `skillz update`: bring installed items to their latest version with the tools that
//! installed them, or with `--check` only say which have a newer one.

use std::io::Write;

use anyhow::bail;
use serde_json::json;

use super::recording::record_since;
use super::removal::{Pick, choose};
use super::scope::{Scene, installed};
use crate::apply::{Applier, Switch, Watch};
use crate::model::{Augmentation, Scope};
use crate::stale::Disk;
use crate::update::check::any_available;
use crate::store::Store;
use crate::{remove, update};

pub(super) struct UpdateRequest<'a> {
    pub query: Option<&'a str>,
    pub yes: bool,
    pub dry_run: bool,
    pub scope: Scope,
}

pub(super) fn update(
    scene: &Scene,
    request: &UpdateRequest,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items: Vec<Augmentation> = scene
        .gather()
        .into_iter()
        .filter(update::can_update)
        .collect();
    if items.is_empty() {
        bail!("nothing here can be updated");
    }
    let queries: Vec<String> = request.query.iter().map(|q| q.to_string()).collect();
    let pick = Pick {
        title: "Select items to update",
        queries: &queries,
        yes: request.yes,
        dry_run: request.dry_run,
    };
    let chosen = choose(scene, &items, &pick)?;
    let switch = Switch::updating(
        chosen
            .into_iter()
            .map(|item| (item.clone(), update::steps(item, &scene.env.project)))
            .collect(),
    );
    print_plan(out, &switch)?;
    if request.dry_run {
        return Ok(());
    }
    if switch.is_empty() || !scene.deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }
    run(scene, (&switch, request.scope), out)
}

fn print_plan(out: &mut dyn Write, switch: &Switch) -> anyhow::Result<()> {
    writeln!(out, "Updating:")?;
    for (item, _) in &switch.updates {
        writeln!(out, "  {}", item.label(true))?;
    }
    writeln!(out, "Plan:")?;
    for step in switch.steps() {
        writeln!(out, "  {}", step.describe())?;
    }
    Ok(())
}

fn run(
    scene: &Scene,
    (switch, scope): (&Switch, Scope),
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let before = installed(deps, env, scope);
    let applier = Applier {
        adapters: &deps.adapters,
        env,
        scope,
        probe: &Disk,
    };
    let done = applier.run_each(
        switch,
        deps.runner,
        &mut Watch {
            step: &mut |_| {},
            item: &mut |_| {},
        },
    );
    let recorded = record_since(deps, &Store::in_env(env), (env, scope), &before);
    writeln!(out, "updated {} of {}", done.updated, switch.updates.len())?;
    if !done.failures.is_empty() {
        bail!("{}", done.failures.join("\n"));
    }
    recorded
}

/// The items matching `query` that have a newer version, as lines or as JSON.
pub(super) fn check(
    scene: &Scene,
    query: Option<&str>,
    json: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.gather();
    let outdated: Vec<&Augmentation> = remove::matches(&items, query)
        .into_iter()
        .filter(|item| any_available(scene.deps.checker.as_ref(), item))
        .collect();
    if json {
        let rows: Vec<_> = outdated
            .iter()
            .map(|i| json!({"id": i.id, "name": i.name, "kind": i.kind, "scope": i.scope}))
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&rows)?)?;
    } else if outdated.is_empty() {
        writeln!(out, "no updates found")?;
    } else {
        outdated
            .iter()
            .try_for_each(|item| writeln!(out, "{}", item.label(true)))?;
    }
    Ok(())
}
