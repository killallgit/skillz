use std::io::Write;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::args::ProfileCommand;
use super::deps::Deps;
use super::scope::{installed, resolve_project};
use crate::model::{Augmentation, Scope};
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
        ProfileCommand::List => list_profiles(&store, out),
        ProfileCommand::Show { name } => Ok(write!(out, "{}", store.load(&name)?.to_text()?)?),
        ProfileCommand::Import { file } => import_profile(&store, &file, out),
        ProfileCommand::Delete { name } => {
            store.load(&name)?;
            store.delete(&name)
        }
        ProfileCommand::Save { name, force } => {
            let items = installed(deps, &env, Scope::Project);
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

fn list_profiles(store: &profile::Store, out: &mut dyn Write) -> anyhow::Result<()> {
    for name in store.names()? {
        let unreadable = if store.load(&name).is_err() {
            "  (unreadable)"
        } else {
            ""
        };
        writeln!(out, "{name}{unreadable}")?;
    }
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
