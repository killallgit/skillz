//! The workspace: pick a project and a profile and apply, unload, add, save or delete, with the
//! plan and progress of each change shown as for `use`.

use std::path::{Path, PathBuf};

use super::deps::Deps;
use super::notice::Notice;
use super::profile::save_profile;
use super::scope::{installed, project_env};
use super::screen::screen_model;
use super::switch::switch;
use crate::model::Scope;
use crate::profile;
use crate::snapshot;
use crate::tui::home::{Act, Pos};
use crate::workspace::Workspace;

/// What an action tells the user in the footer, if anything.
type Reply = Option<Notice>;

/// The projects, profiles and prompts one workspace session works with.
struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    store: profile::Store,
    registry: Workspace,
}

/// A step that fails shows its error in the footer and the workspace stays open. The terminal
/// is entered once and kept until the workspace closes.
pub fn workspace(deps: &Deps) -> anyhow::Result<()> {
    let _held = deps.prompter.hold();
    let base = project_env(deps, &deps.cwd);
    let session = Session {
        deps,
        store: profile::Store::in_env(&base),
        registry: Workspace::in_env(&base),
    };
    let mut pos = Pos::default();
    let mut notice = None;
    loop {
        let model = screen_model(deps, &session.store, &session.registry, notice.take())?;
        let (act, next) = deps.prompter.home(model, pos)?;
        pos = next;
        if act == Act::Quit {
            return Ok(());
        }
        notice = session
            .perform(act)
            .unwrap_or_else(|e| Some(Notice::Failed(format!("{e:#}"))));
    }
}

impl Session<'_, '_> {
    fn perform(&self, act: Act) -> anyhow::Result<Reply> {
        match act {
            Act::Quit => Ok(None),
            Act::Apply { project, profile } => self.switch(&project, Some(&profile)),
            Act::Unload { project } => self.switch(&project, None),
            Act::AddProject(text) => self.add_project(text),
            Act::ForgetProject(dir) => self.forget_project(&dir),
            Act::NewProfile(name) => self.new_profile(name),
            Act::SaveProfile { name, project } => self.save_setup(name, &project),
            Act::DeleteProfile(name) => self.delete_profile(&name),
            Act::AddInstalled { profile, project } => self.add_installed(&profile, &project),
            Act::RemoveEntries { profile } => self.remove_entries(&profile),
            Act::AddTyped { profile, spec } => self.add_typed(&profile, &spec),
        }
    }

    /// The registered project at `dir`; an error when it was forgotten in the meantime.
    fn project(&self, dir: &Path) -> anyhow::Result<PathBuf> {
        let known = self.registry.projects()?.into_iter().any(|p| p == dir);
        anyhow::ensure!(known, "{} is not a registered project", dir.display());
        Ok(dir.to_path_buf())
    }

    /// The stored profile `name`; an error when it was deleted in the meantime.
    fn profile(&self, name: &str) -> anyhow::Result<String> {
        anyhow::ensure!(
            self.store.names()?.iter().any(|n| n == name),
            "profile {name} does not exist"
        );
        Ok(name.to_string())
    }

    fn switch(&self, dir: &Path, profile: Option<&str>) -> anyhow::Result<Reply> {
        let dir = self.project(dir)?;
        let profile = profile.map(|name| self.profile(name)).transpose()?;
        switch(self.deps, &dir, profile.as_deref())
    }

    fn add_project(&self, text: String) -> anyhow::Result<Reply> {
        self.registry.add(&self.deps.cwd.join(text))?;
        Ok(None)
    }

    fn forget_project(&self, dir: &Path) -> anyhow::Result<Reply> {
        self.registry.remove(&self.project(dir)?)?;
        Ok(None)
    }

    fn new_profile(&self, name: String) -> anyhow::Result<Reply> {
        profile::check_name(&name)?;
        anyhow::ensure!(
            !self.store.names()?.contains(&name),
            "profile {name} exists"
        );
        self.store.save(&profile::Profile::new(&name))?;
        Ok(None)
    }

    fn save_setup(&self, name: String, project: &Path) -> anyhow::Result<Reply> {
        let dir = self.project(project)?;
        let items = installed(self.deps, &project_env(self.deps, &dir), Scope::Project);
        let snap = save_profile(&self.store, &items, &name, false)?;
        Ok(Some(Notice::Saved {
            name,
            entries: snap.profile.entries.len(),
            skipped: snap.skipped.iter().map(|s| s.reason).collect(),
        }))
    }

    fn delete_profile(&self, name: &str) -> anyhow::Result<Reply> {
        self.store.delete(&self.profile(name)?)?;
        Ok(None)
    }

    /// Offer what the project has installed that the profile does not have yet.
    fn add_installed(&self, profile: &str, project: &Path) -> anyhow::Result<Reply> {
        let (name, dir) = (self.profile(profile)?, self.project(project)?);
        let mut p = self.store.load(&name)?;
        let items = installed(self.deps, &project_env(self.deps, &dir), Scope::Project);
        let have = p.resolved();
        let offered: Vec<profile::Entry> = snapshot::snapshot(&name, &items)
            .profile
            .entries
            .into_iter()
            .filter(|e| !have.contains(e))
            .collect();
        let picked = self.pick_entries(&format!("Add to {name}"), &offered)?;
        let added = picked.len();
        p.entries
            .extend(picked.into_iter().map(|i| offered[i].clone()));
        self.store.save(&p)?;
        Ok(Some(Notice::Added { name, count: added }))
    }

    fn remove_entries(&self, profile: &str) -> anyhow::Result<Reply> {
        let name = self.profile(profile)?;
        let mut p = self.store.load(&name)?;
        let picked = self.pick_entries(&format!("Remove from {name}"), &p.entries)?;
        let mut index = 0;
        p.entries.retain(|_| {
            index += 1;
            !picked.contains(&(index - 1))
        });
        self.store.save(&p)?;
        Ok(Some(Notice::Removed {
            name,
            count: picked.len(),
        }))
    }

    fn add_typed(&self, profile: &str, spec: &str) -> anyhow::Result<Reply> {
        let mut p = self.store.load(&self.profile(profile)?)?;
        p.entries.push(profile::Entry::parse_spec(spec)?);
        self.store.save(&p)?;
        Ok(None)
    }

    /// Indices of the entries the user marks, valid for `entries`.
    fn pick_entries(&self, title: &str, entries: &[profile::Entry]) -> anyhow::Result<Vec<usize>> {
        let labels: Vec<String> = entries.iter().map(ToString::to_string).collect();
        let picked = self.deps.prompter.multi_select(title, &labels)?;
        Ok(picked.into_iter().filter(|&i| i < entries.len()).collect())
    }
}
