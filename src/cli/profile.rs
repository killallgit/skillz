use std::io::Write;

use anyhow::bail;

use super::args::{Filter, ProfileCommand};
use super::deps::Deps;
use super::scope::{gather, resolve, single_scope};
use crate::env::Env;
use crate::model::Scope;
use crate::{profile, snapshot};

pub(super) fn profile_command(
    deps: &Deps,
    action: ProfileCommand,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let home_env = Env::new(deps.home.clone(), deps.cwd.clone());
    let store = profile::Store::in_env(&home_env);
    match action {
        ProfileCommand::List => {
            for name in store.names()? {
                match store.load(&name) {
                    Ok(p) => writeln!(out, "{name}  ({} entries)", p.entries.len())?,
                    Err(e) => writeln!(out, "{name}  (unreadable: {e:#})")?,
                }
            }
        }
        ProfileCommand::Show { name } => write!(out, "{}", store.load(&name)?.to_text()?)?,
        ProfileCommand::Delete { name } => {
            store.load(&name)?;
            store.delete(&name)?;
        }
        ProfileCommand::Save {
            name,
            global,
            project,
            force,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let scope = single_scope(&scopes, global);
            let snap = save_profile(deps, &store, &env, scope, &name, force)?;
            writeln!(out, "saved {name} from the {scope} config:")?;
            for entry in &snap.profile.entries {
                writeln!(out, "  {entry}")?;
            }
            for label in &snap.skipped {
                writeln!(out, "  not saved (nothing to reinstall it from): {label}")?;
            }
        }
    }
    Ok(())
}

/// Snapshot what `scope` of `env` has installed into the profile `name`.
pub(super) fn save_profile(
    deps: &Deps,
    store: &profile::Store,
    env: &Env,
    scope: Scope,
    name: &str,
    force: bool,
) -> anyhow::Result<snapshot::Snapshot> {
    profile::check_name(name)?;
    if !force && store.names()?.iter().any(|n| n == name) {
        bail!("profile {name} exists; use --force to replace it");
    }
    let items = gather(deps, env, &[scope], &Filter::default());
    let snap = snapshot::snapshot(name, &items);
    store.save(&snap.profile)?;
    Ok(snap)
}
