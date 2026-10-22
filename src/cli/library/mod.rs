//! `skillz library`: the library the workspace's Library tab shows, from the command line.

mod install;
mod rows;

use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;

use super::deps::Deps;
use super::library_args::{AddArgs, LibraryCommand, LibraryListArgs, LibraryRemoveArgs};
use super::pick::{Choosing, pick};
use super::scope::{installed, resolve_project};
use super::session::{Console, Session, with_session};
use crate::env::Env;
use crate::library::{Catalog, Held, LibraryItem};
use crate::model::Scope;
use crate::profile::Entry;
pub(super) use rows::{Row, rows};

pub(super) fn library_command(
    deps: &Deps,
    action: LibraryCommand,
    project: Option<PathBuf>,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let env = resolve_project(deps, project.as_deref())?;
    match action {
        LibraryCommand::List(args) => list(deps, &env, &args, out),
        LibraryCommand::Add(args) => {
            with_session(deps, &env, out, args.dry_run, |session, console| {
                add(session, console, &args)
            })
        }
        LibraryCommand::Remove(args) => {
            with_session(deps, &env, out, args.dry_run, |session, console| {
                remove(session, console, &args)
            })
        }
        LibraryCommand::Install(args) => {
            with_session(deps, &env, out, args.dry_run, |session, console| {
                install::install(session, console, &args)
            })
        }
    }
}

/// The library kept in the store, and what the project and the user have installed of it.
pub(super) fn held_by(deps: &Deps, env: &Env) -> Held {
    let [project, global] =
        [Scope::Project, Scope::Global].map(|scope| installed(deps, env, scope));
    Held::of(&project, &global)
}

fn list(deps: &Deps, env: &Env, args: &LibraryListArgs, out: &mut dyn Write) -> anyhow::Result<()> {
    let library = crate::store::Store::in_env(env).read()?.items;
    let kinds = args.kinds.kinds();
    let shown = library
        .items()
        .iter()
        .filter(|item| kinds.is_empty() || kinds.contains(&item.kind()))
        .collect();
    let rows = rows(shown, &held_by(deps, env));
    if args.json {
        let json: Vec<_> = rows.iter().map(Row::json).collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&json)?)?;
    } else {
        rows.iter()
            .try_for_each(|row| writeln!(out, "{}", row.line()))?;
    }
    Ok(())
}

fn add(session: &Session, console: &Console, args: &AddArgs) -> anyhow::Result<()> {
    let spec = &args.spec;
    if args.dry_run {
        let entry = Entry::parse_spec(spec)?;
        return console.say(&format!("would add {entry} to the library"));
    }
    console.conclude(session.library_add(spec)?)
}

fn remove(session: &Session, console: &Console, args: &LibraryRemoveArgs) -> anyhow::Result<()> {
    let library = session.library().read()?.items;
    let items = the_items(console, &library, &args.item, args.yes)?;
    for item in items {
        if args.dry_run {
            console.say(&format!("would remove {} from the library", item.label))?;
        } else {
            console.conclude(session.library_remove(&item.entry)?)?;
        }
    }
    Ok(())
}

/// The library item a line describes exactly, else the items a query picks.
fn the_items(
    console: &Console,
    library: &Catalog,
    text: &str,
    yes: bool,
) -> anyhow::Result<Vec<LibraryItem>> {
    if let Ok(entry) = Entry::parse_spec(text) {
        let wanted = LibraryItem::from_entry(entry);
        let held = library.items().iter().find(|i| i.is_same_thing_as(&wanted));
        return Ok(vec![
            held.cloned().context("that item is not in the library")?,
        ]);
    }
    let choosing = Choosing {
        prompter: console,
        title: "Select the item to remove",
        yes,
    };
    let picked = pick(&choosing, text, library.items(), |i| i.label.clone())?;
    Ok(picked.into_iter().cloned().collect())
}
