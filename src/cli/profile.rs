use std::io::Write;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::deps::Deps;
use super::profile_args::ProfileCommand;
use super::profile_entries::{add, remove};
use super::profile_status::{list, status};
use super::scope::{complete_installed, resolve_project};
use super::session::with_session;
use crate::model::{Installed, Scope};
use crate::{profile, snapshot};

pub(super) fn profile_command(
    deps: &Deps,
    action: ProfileCommand,
    project: Option<PathBuf>,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let env = resolve_project(deps, project.as_deref())?;
    let store = profile::Store::in_env(&env);
    match action {
        ProfileCommand::List { json } => list(deps, &env, json, out),
        ProfileCommand::Status { name, json } => status(deps, &env, (name.as_deref(), json), out),
        ProfileCommand::New { name } => with_session(deps, &env, out, false, |session, console| {
            console.conclude(Some(session.create_profile(&name, &[])?))
        }),
        ProfileCommand::Add(args) => {
            with_session(deps, &env, out, args.dry_run, |session, console| {
                add(session, console, &args)
            })
        }
        ProfileCommand::Remove(args) => {
            with_session(deps, &env, out, args.dry_run, |session, console| {
                remove(session, console, &args)
            })
        }
        ProfileCommand::Show { name } => Ok(write!(out, "{}", store.load(&name)?.to_text()?)?),
        ProfileCommand::Import { file } => import_profile(&store, &file, out),
        ProfileCommand::Delete { name } => {
            store.load(&name)?;
            with_session(deps, &env, out, false, |session, console| {
                console.conclude(Some(session.delete_profile(&name)?))
            })
        }
        ProfileCommand::Save { name, force } => {
            let items = complete_installed(deps, &env, Scope::Project)?;
            let snap = save_profile(&store, &items, &name, force)?;
            print_saved(out, &name, &snap)
        }
    }
}

fn import_profile(store: &profile::Store, file: &Path, out: &mut dyn Write) -> anyhow::Result<()> {
    let text = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let profile = profile::Profile::from_text(&text)?;
    store.save(&profile)?;
    writeln!(out, "imported {}", profile.name)?;
    Ok(())
}

fn print_saved(out: &mut dyn Write, name: &str, snap: &snapshot::Snapshot) -> anyhow::Result<()> {
    writeln!(out, "saved {name} from the project config:")?;
    for entry in &snap.profile.entries {
        writeln!(out, "  {entry}")?;
    }
    for skipped in &snap.skipped {
        writeln!(out, "  not saved ({}): {}", skipped.reason, skipped.label)?;
    }
    Ok(())
}

/// Snapshot `items` into the profile `name`; an existing one is replaced only with `force`.
pub(super) fn save_profile(
    store: &profile::Store,
    items: &[Installed],
    name: &str,
    force: bool,
) -> anyhow::Result<snapshot::Snapshot> {
    profile::check_name(name)?;
    if !force && store.names()?.iter().any(|n| n == name) {
        bail!("profile {name} exists; use --force to replace it");
    }
    let snap = snapshot::snapshot(name, items);
    store.save(&snap.profile)?;
    Ok(snap)
}
