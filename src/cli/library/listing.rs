//! The library listed: all of it, or what a scope lacks of it.

use std::io::Write;

use super::super::deps::Deps;
use super::super::library_args::LibraryListArgs;
use super::super::scope::installed;
use super::rows::{Row, rows};
use crate::app::apply::holds;
use crate::app::library::{Held, LibraryItem};
use crate::app::stale::Disk;
use crate::app::store::Store;
use skillz_harness::env::Env;
use skillz_harness::model::Scope;

/// What the project and the user have installed, as the library tells it apart.
fn held_by(deps: &Deps, env: &Env) -> Held {
    let [project, global] =
        [Scope::Project, Scope::Global].map(|scope| installed(deps, env, scope));
    Held::of(&project, &global)
}

pub(super) fn list(
    deps: &Deps,
    env: &Env,
    args: &LibraryListArgs,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let library = Store::in_env(env).read()?.items.in_env(env);
    let kinds = args.kinds.kinds();
    let shown: Vec<&LibraryItem> = library
        .items()
        .iter()
        .filter(|item| kinds.is_empty() || kinds.contains(&item.kind()))
        .collect();
    write_rows(rows(shown, (&held_by(deps, env), &Disk)), args.json, out)
}

/// The library items the project, or with `Scope::Global` the user, does not have installed.
pub(in crate::cli) fn available(
    deps: &Deps,
    env: &Env,
    (scope, json): (Scope, bool),
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let library = Store::in_env(env).read()?.items.in_env(env);
    let here = installed(deps, env, scope);
    let offered: Vec<&LibraryItem> = library
        .items()
        .iter()
        .filter(|item| !holds(&here, &item.entry))
        .collect();
    write_rows(rows(offered, (&held_by(deps, env), &Disk)), json, out)
}

fn write_rows(rows: Vec<Row>, json: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    if json {
        let json: Vec<_> = rows.iter().map(Row::json).collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&json)?)?;
    } else {
        rows.iter()
            .try_for_each(|row| writeln!(out, "{}", row.line()))?;
    }
    Ok(())
}
