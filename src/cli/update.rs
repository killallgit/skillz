//! `skillz update`: bring installed items to their latest version with the tools that
//! installed them, or with `--check` only say which have a newer one.

use std::io::Write;

use anyhow::bail;
use serde_json::json;

use super::removal::{Pick, choose};
use super::scope::Scene;
use super::session::{Change, change, with_session};
use crate::model::{Augmentation, Scope};
use crate::update::check::any_available;
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
        .complete()?
        .into_iter()
        .filter(|item| !update::steps(&scene.deps.adapters, item, &scene.env.project).is_empty())
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
    let ids: Vec<String> = chosen
        .into_iter()
        .flat_map(|item| item.parts().iter().map(|part| part.id.clone()))
        .collect();
    with_session(
        scene.deps,
        scene.env,
        out,
        request.dry_run,
        |session, console| console.conclude(change(session, request.scope, &Change::Update(&ids))?),
    )
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
