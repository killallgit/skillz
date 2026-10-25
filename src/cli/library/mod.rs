//! `skillz library`: the library the workspace's Library tab shows, from the command line.

mod install;
mod listing;
mod rows;

use crate::profile::SpecParse;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;

use super::deps::Deps;
use super::library_args::{AddArgs, LibraryCommand, LibraryRemoveArgs};
use super::pick::{Choosing, pick};
use super::scope::resolve_project;
use super::session::{Console, Session, with_session};
use crate::library::{Catalog, LibraryItem};
pub(super) use listing::available;
use skillz_harness::Spec;

pub(super) fn library_command(
    deps: &Deps,
    action: LibraryCommand,
    project: Option<PathBuf>,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let env = resolve_project(deps, project.as_deref())?;
    match action {
        LibraryCommand::List(args) => listing::list(deps, &env, &args, out),
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

fn add(session: &Session, console: &Console, args: &AddArgs) -> anyhow::Result<()> {
    let spec = &args.spec;
    if args.dry_run {
        let entry = Spec::parse_spec(spec)?;
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
    if let Ok(entry) = Spec::parse_spec(text) {
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
