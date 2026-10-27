//! The library listed: all of it, or what a scope lacks of it.

use std::io::Write;

use super::super::deps::Deps;
use super::super::library_args::LibraryListArgs;
use super::super::scope::installed;
use super::rows::{Row, grouped, rows};
use crate::app::apply::holds;
use crate::app::json::Json;
use crate::app::library::{Held, LibraryItem};
use crate::app::stale::Disk;
use crate::app::store::Store;
use skillz_harness::env::Env;
use skillz_harness::model::Scope;
use std::path::Path;

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
    let rows = rows(shown, (&held_by(deps, env), &Disk));
    write_rows(&rows, (args.json, &deps.home), out)
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
    let rows = rows(offered, (&held_by(deps, env), &Disk));
    write_rows(&rows, (json, &deps.home), out)
}

/// Columns of the rules over each group in the printed listing.
const WIDTH: usize = 72;

/// JSON keeps every path as it is; the listing a person reads groups the items by package and
/// writes the home directory `~`.
fn write_rows(
    rows: &[Row],
    (json, home): (bool, &Path),
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    if json {
        let json: Vec<_> = rows.iter().map(Row::json).collect();
        json.write_json(out)?;
    } else {
        grouped(rows, home, WIDTH)
            .iter()
            .try_for_each(|line| writeln!(out, "{line}"))?;
    }
    Ok(())
}
