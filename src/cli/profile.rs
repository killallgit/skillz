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

/// One stored profile: its entry count, or why it could not be read.
#[derive(Debug, PartialEq, Eq)]
struct ProfileRow {
    name: String,
    summary: Result<usize, String>,
}

fn profile_row(name: String, loaded: anyhow::Result<profile::Profile>) -> ProfileRow {
    let summary = loaded
        .map(|p| p.entries.len())
        .map_err(|e| format!("{e:#}"));
    ProfileRow { name, summary }
}

fn profile_rows(store: &profile::Store) -> anyhow::Result<Vec<ProfileRow>> {
    Ok(store
        .names()?
        .into_iter()
        .map(|name| {
            let loaded = store.load(&name);
            profile_row(name, loaded)
        })
        .collect())
}

fn render_profiles(rows: &[ProfileRow], out: &mut dyn Write) -> anyhow::Result<()> {
    for ProfileRow { name, summary } in rows {
        match summary {
            Ok(entries) => writeln!(out, "{name}  ({entries} entries)")?,
            Err(e) => writeln!(out, "{name}  (unreadable: {e})")?,
        }
    }
    Ok(())
}

fn list_profiles(store: &profile::Store, out: &mut dyn Write) -> anyhow::Result<()> {
    render_profiles(&profile_rows(store)?, out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_row_counts_entries_or_keeps_the_load_error() {
        let ok = profile_row("a".into(), Ok(profile::Profile::new("a")));
        let bad = profile_row("b".into(), Err(anyhow::anyhow!("boom")));

        // Assert the readable profile has a count and the broken one carries its error.
        assert_eq!(ok.summary, Ok(0));
        assert!(bad.summary.is_err());
    }
}
