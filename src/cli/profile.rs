use std::io::Write;

use anyhow::bail;

use super::args::ProfileCommand;
use super::deps::Deps;
use super::scope::{installed, project_env, resolve, single_scope};
use crate::model::{Augmentation, Scope};
use crate::{profile, snapshot};

pub(super) fn profile_command(
    deps: &Deps,
    action: ProfileCommand,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let store = profile::Store::in_env(&project_env(deps, &deps.cwd));
    match action {
        ProfileCommand::List => list_profiles(&store, out),
        ProfileCommand::Show { name } => Ok(write!(out, "{}", store.load(&name)?.to_text()?)?),
        ProfileCommand::Delete { name } => {
            store.load(&name)?;
            store.delete(&name)
        }
        ProfileCommand::Save {
            name,
            global,
            project,
            force,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let scope = single_scope(&scopes, global);
            let items = installed(deps, &env, scope);
            let snap = save_profile(&store, &items, &name, force)?;
            print_saved(out, &name, scope, &snap)
        }
    }
}

fn list_profiles(store: &profile::Store, out: &mut dyn Write) -> anyhow::Result<()> {
    for name in store.names()? {
        match store.load(&name) {
            Ok(p) => writeln!(out, "{name}  ({} entries)", p.entries.len())?,
            Err(e) => writeln!(out, "{name}  (unreadable: {e:#})")?,
        }
    }
    Ok(())
}

fn print_saved(
    out: &mut dyn Write,
    name: &str,
    scope: Scope,
    snap: &snapshot::Snapshot,
) -> anyhow::Result<()> {
    writeln!(out, "saved {name} from the {scope} config:")?;
    for entry in &snap.profile.entries {
        writeln!(out, "  {entry}")?;
    }
    for label in &snap.skipped {
        writeln!(out, "  not saved (nothing to reinstall it from): {label}")?;
    }
    Ok(())
}

/// Snapshot `items` into the profile `name`; an existing one is replaced only with `force`.
pub(super) fn save_profile(
    store: &profile::Store,
    items: &[Augmentation],
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
