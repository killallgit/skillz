use std::io::Write;
use std::path::Path;

use super::args::Filter;
use super::deps::Deps;
use super::profile::save_profile;
use super::scope::{gather, project_env};
use super::use_profile::use_profile;
use crate::model::Scope;
use crate::tui::home;
use crate::workspace::Workspace;
use crate::{apply, profile, snapshot};

pub(super) fn workspace_model(
    deps: &Deps,
    store: &profile::Store,
    workspace: &Workspace,
    notice: Option<String>,
) -> anyhow::Result<home::Model> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    let short = |p: &Path| match p.strip_prefix(&deps.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    };
    let projects = workspace
        .projects()?
        .into_iter()
        .map(|path| {
            let env = project_env(deps, &path);
            let record = ledger
                .get(&apply::Ledger::key(&env, Scope::Project))
                .ok()
                .flatten();
            let missing = !path.is_dir();
            let profile_name = record.as_ref().map_or("-", |r| r.profile.as_str());
            let mut detail = vec![path.display().to_string()];
            if missing {
                detail.push("directory is missing".into());
            }
            for loaded in record.iter().flat_map(|r| &r.entries) {
                detail.push(format!("{}", loaded.entry));
                detail.extend(loaded.items.iter().map(|o| format!("    {}", o.name)));
            }
            home::ProjectRow {
                label: format!(
                    "{profile_name:<14} {}{}",
                    short(&path),
                    if missing { "  (missing)" } else { "" }
                ),
                loaded: record.is_some(),
                missing,
                detail,
                path,
            }
        })
        .collect();
    let profiles = store
        .names()?
        .into_iter()
        .map(|name| match store.load(&name) {
            Ok(p) => home::ProfileRow {
                detail: p.entries.iter().map(ToString::to_string).collect(),
                broken: false,
                name,
            },
            Err(e) => home::ProfileRow {
                detail: vec![format!("{e:#}")],
                broken: true,
                name,
            },
        })
        .collect();
    Ok(home::Model {
        projects,
        profiles,
        notice,
    })
}

/// What the workspace does after a step.
enum Flow {
    Stay,
    Leave,
}

/// The workspace: pick a project and a profile and apply, unload, add, save or delete, with the
/// plan and progress of each change shown as for `use`. A step that fails shows its error in the
/// footer and the workspace stays open.
pub fn workspace(deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    let base = project_env(deps, &deps.cwd);
    let store = profile::Store::in_env(&base);
    let registry = Workspace::in_env(&base);
    let mut pos = home::Pos::default();
    let mut notice = None;
    loop {
        let model = workspace_model(deps, &store, &registry, notice.take())?;
        let (act, next) = deps.prompter.home(model, pos)?;
        pos = next;
        match workspace_step(deps, &store, &registry, act, &mut notice, out) {
            Ok(Flow::Leave) => return Ok(()),
            Ok(Flow::Stay) => {}
            Err(e) => notice = Some(format!("{e:#}")),
        }
    }
}

fn workspace_step(
    deps: &Deps,
    store: &profile::Store,
    registry: &Workspace,
    act: home::Act,
    notice: &mut Option<String>,
    out: &mut dyn Write,
) -> anyhow::Result<Flow> {
    use home::Act;
    let projects = registry.projects()?;
    let names = store.names()?;
    let switch =
        |project: usize, name: Option<&str>, out: &mut dyn Write| -> anyhow::Result<Flow> {
            let Some(dir) = projects.get(project) else {
                return Ok(Flow::Stay);
            };
            use_profile(
                deps,
                &project_env(deps, dir),
                Scope::Project,
                name,
                false,
                out,
            )?;
            out.flush()?;
            Ok(if deps.prompter.confirm("Back to the workspace?")? {
                Flow::Stay
            } else {
                Flow::Leave
            })
        };
    match act {
        Act::Quit => return Ok(Flow::Leave),
        Act::Apply { project, profile } => {
            let name = names.get(profile).map(String::as_str);
            return switch(project, name, out);
        }
        Act::Unload { project } => return switch(project, None, out),
        Act::AddProject(text) => {
            registry.add(&deps.cwd.join(text))?;
        }
        Act::ForgetProject(i) => {
            if let Some(dir) = projects.get(i) {
                registry.remove(dir)?;
            }
        }
        Act::NewProfile(name) => {
            profile::check_name(&name)?;
            anyhow::ensure!(!names.contains(&name), "profile {name} exists");
            store.save(&profile::Profile::new(&name))?;
        }
        Act::SaveProfile { name, project } => {
            if let Some(dir) = projects.get(project) {
                let env = project_env(deps, dir);
                let snap = save_profile(deps, store, &env, Scope::Project, &name, false)?;
                *notice = Some(format!(
                    "saved {name}: {} entries, {} not reproducible",
                    snap.profile.entries.len(),
                    snap.skipped.len()
                ));
            }
        }
        Act::DeleteProfile(i) => {
            if let Some(name) = names.get(i) {
                store.delete(name)?;
            }
        }
        Act::AddInstalled { profile, project } => {
            let (Some(name), Some(dir)) = (names.get(profile), projects.get(project)) else {
                return Ok(Flow::Stay);
            };
            let mut p = store.load(name)?;
            let items = gather(
                deps,
                &project_env(deps, dir),
                &[Scope::Project],
                &Filter::default(),
            );
            let have = p.resolved();
            let offered: Vec<profile::Entry> = snapshot::snapshot(name, &items)
                .profile
                .entries
                .into_iter()
                .filter(|e| !have.contains(e))
                .collect();
            let labels: Vec<String> = offered.iter().map(ToString::to_string).collect();
            let picked = deps
                .prompter
                .multi_select(&format!("Add to {name}"), &labels)?;
            let added = picked.len();
            p.entries
                .extend(picked.into_iter().filter_map(|i| offered.get(i).cloned()));
            store.save(&p)?;
            *notice = Some(format!("added {added} entries to {name}"));
        }
        Act::RemoveEntries { profile } => {
            let Some(name) = names.get(profile) else {
                return Ok(Flow::Stay);
            };
            let mut p = store.load(name)?;
            let labels: Vec<String> = p.entries.iter().map(ToString::to_string).collect();
            let picked = deps
                .prompter
                .multi_select(&format!("Remove from {name}"), &labels)?;
            let mut index = 0;
            p.entries.retain(|_| {
                index += 1;
                !picked.contains(&(index - 1))
            });
            store.save(&p)?;
            *notice = Some(format!("removed {} entries from {name}", picked.len()));
        }
        Act::AddTyped { profile, spec } => {
            let Some(name) = names.get(profile) else {
                return Ok(Flow::Stay);
            };
            let mut p = store.load(name)?;
            p.entries.push(profile::Entry::parse_spec(&spec)?);
            store.save(&p)?;
        }
    }
    Ok(Flow::Stay)
}
