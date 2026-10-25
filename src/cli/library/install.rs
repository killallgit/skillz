//! `library install`: put library items in the project or at user level as one plan.

use super::super::library_args::LibraryInstallArgs;
use super::super::pick::{Choosing, pick};
use super::super::scope::{installed, project_env};
use super::super::session::{Change, Console, Selection, Session, change};
use crate::apply::holds;
use crate::harness::Spec;
use crate::harness::model::Scope;
use crate::prompt::Prompter;

pub(super) fn install(
    session: &Session,
    console: &Console,
    args: &LibraryInstallArgs,
) -> anyhow::Result<()> {
    let scope = if args.global {
        Scope::Global
    } else {
        Scope::Project
    };
    let wanted = picked_entries(session, console, args)?;
    let entries = match scope {
        Scope::Project => without_those_held_globally(session, console, wanted)?,
        Scope::Global => wanted,
    };
    let selection = Selection {
        install: &entries,
        remove: &[],
        only: None,
    };
    console.conclude(change(session, scope, &Change::Selection(&selection))?)
}

fn picked_entries(
    session: &Session,
    console: &Console,
    args: &LibraryInstallArgs,
) -> anyhow::Result<Vec<Spec>> {
    let env = project_env(session.deps(), session.project());
    let library = session.library().read()?.items.in_env(&env);
    let choosing = Choosing {
        prompter: console,
        title: "Select items to install",
        yes: args.yes,
    };
    let mut entries: Vec<Spec> = Vec::new();
    for query in &args.queries {
        for item in pick(&choosing, query, library.items(), |i| i.label.clone())? {
            if !entries.contains(&item.entry) {
                entries.push(item.entry.clone());
            }
        }
    }
    Ok(entries)
}

/// Installing into the project what the user has installed already is asked about first.
fn without_those_held_globally(
    session: &Session,
    console: &Console,
    wanted: Vec<Spec>,
) -> anyhow::Result<Vec<Spec>> {
    let env = project_env(session.deps(), session.project());
    let global = installed(session.deps(), &env, Scope::Global);
    let (held, rest): (Vec<Spec>, Vec<Spec>) =
        wanted.into_iter().partition(|entry| holds(&global, entry));
    let names: Vec<String> = held.iter().map(ToString::to_string).collect();
    let also = !held.is_empty()
        && console.confirm(&format!(
            "Already installed globally: {}. Also install into this project?",
            names.join(", ")
        ))?;
    Ok(if also { [rest, held].concat() } else { rest })
}
