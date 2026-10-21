//! The workspace: pick a project and a profile and apply, unload, add, save or delete, with the
//! plan and progress of each change shown as for `use`.

use std::io::Write;
use std::path::PathBuf;

use super::deps::Deps;
use super::notice::Notice;
use super::profile::save_profile;
use super::scope::{installed, project_env};
use super::screen::screen_model;
use super::use_profile::{Switching, Target, use_profile};
use crate::model::Scope;
use crate::profile;
use crate::snapshot;
use crate::tui::home::{Act, Pos};
use crate::workspace::Workspace;

/// What the workspace does after an action.
enum Flow {
    Stay,
    Leave,
}

/// An action's result: whether to carry on, and what to tell the user in the footer.
struct Reply {
    flow: Flow,
    notice: Option<Notice>,
}

impl Reply {
    fn done() -> Self {
        Reply {
            flow: Flow::Stay,
            notice: None,
        }
    }

    fn said(notice: Notice) -> Self {
        Reply {
            flow: Flow::Stay,
            notice: Some(notice),
        }
    }

    fn leave() -> Self {
        Reply {
            flow: Flow::Leave,
            notice: None,
        }
    }
}

/// The projects, profiles and prompts one workspace session works with.
struct Session<'a, 'd> {
    deps: &'a Deps<'d>,
    store: profile::Store,
    registry: Workspace,
}

/// A step that fails shows its error in the footer and the workspace stays open.
pub fn workspace(deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
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
        match session.perform(act, out) {
            Ok(Reply {
                flow: Flow::Leave, ..
            }) => return Ok(()),
            Ok(reply) => notice = reply.notice,
            Err(e) => notice = Some(Notice::Failed(format!("{e:#}"))),
        }
    }
}

impl Session<'_, '_> {
    fn perform(&self, act: Act, out: &mut dyn Write) -> anyhow::Result<Reply> {
        match act {
            Act::Quit => Ok(Reply::leave()),
            Act::Apply { project, profile } => self.switch(project, self.profile(profile), out),
            Act::Unload { project } => self.switch(project, None, out),
            Act::AddProject(text) => self.add_project(text),
            Act::ForgetProject(i) => self.forget_project(i),
            Act::NewProfile(name) => self.new_profile(name),
            Act::SaveProfile { name, project } => self.save_setup(name, project),
            Act::DeleteProfile(i) => self.delete_profile(i),
            Act::AddInstalled { profile, project } => self.add_installed(profile, project),
            Act::RemoveEntries { profile } => self.remove_entries(profile),
            Act::AddTyped { profile, spec } => self.add_typed(profile, &spec),
        }
    }

    fn project(&self, i: usize) -> Option<PathBuf> {
        self.registry.projects().ok()?.into_iter().nth(i)
    }

    fn profile(&self, i: usize) -> Option<String> {
        self.store.names().ok()?.into_iter().nth(i)
    }

    /// Load `profile` into the project (or unload with `None`), then ask whether to come back.
    fn switch(
        &self,
        project: usize,
        profile: Option<String>,
        out: &mut dyn Write,
    ) -> anyhow::Result<Reply> {
        let Some(dir) = self.project(project) else {
            return Ok(Reply::done());
        };
        let env = project_env(self.deps, &dir);
        let target = Target {
            env: &env,
            scope: Scope::Project,
        };
        let switching = Switching {
            profile: profile.as_deref(),
            dry_run: false,
        };
        use_profile(self.deps, &target, &switching, out)?;
        out.flush()?;
        let back = self.deps.prompter.confirm("Back to the workspace?")?;
        Ok(if back { Reply::done() } else { Reply::leave() })
    }

    fn add_project(&self, text: String) -> anyhow::Result<Reply> {
        self.registry.add(&self.deps.cwd.join(text))?;
        Ok(Reply::done())
    }

    fn forget_project(&self, i: usize) -> anyhow::Result<Reply> {
        if let Some(dir) = self.project(i) {
            self.registry.remove(&dir)?;
        }
        Ok(Reply::done())
    }

    fn new_profile(&self, name: String) -> anyhow::Result<Reply> {
        profile::check_name(&name)?;
        anyhow::ensure!(
            !self.store.names()?.contains(&name),
            "profile {name} exists"
        );
        self.store.save(&profile::Profile::new(&name))?;
        Ok(Reply::done())
    }

    fn save_setup(&self, name: String, project: usize) -> anyhow::Result<Reply> {
        let Some(dir) = self.project(project) else {
            return Ok(Reply::done());
        };
        let items = installed(self.deps, &project_env(self.deps, &dir), Scope::Project);
        let snap = save_profile(&self.store, &items, &name, false)?;
        Ok(Reply::said(Notice::Saved {
            name,
            entries: snap.profile.entries.len(),
            skipped: snap.skipped.iter().map(|s| s.reason).collect(),
        }))
    }

    fn delete_profile(&self, i: usize) -> anyhow::Result<Reply> {
        if let Some(name) = self.profile(i) {
            self.store.delete(&name)?;
        }
        Ok(Reply::done())
    }

    /// Offer what the project has installed that the profile does not have yet.
    fn add_installed(&self, profile: usize, project: usize) -> anyhow::Result<Reply> {
        let (Some(name), Some(dir)) = (self.profile(profile), self.project(project)) else {
            return Ok(Reply::done());
        };
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
        Ok(Reply::said(Notice::Added { name, count: added }))
    }

    fn remove_entries(&self, profile: usize) -> anyhow::Result<Reply> {
        let Some(name) = self.profile(profile) else {
            return Ok(Reply::done());
        };
        let mut p = self.store.load(&name)?;
        let picked = self.pick_entries(&format!("Remove from {name}"), &p.entries)?;
        let mut index = 0;
        p.entries.retain(|_| {
            index += 1;
            !picked.contains(&(index - 1))
        });
        self.store.save(&p)?;
        Ok(Reply::said(Notice::Removed {
            name,
            count: picked.len(),
        }))
    }

    fn add_typed(&self, profile: usize, spec: &str) -> anyhow::Result<Reply> {
        let Some(name) = self.profile(profile) else {
            return Ok(Reply::done());
        };
        let mut p = self.store.load(&name)?;
        p.entries.push(profile::Entry::parse_spec(spec)?);
        self.store.save(&p)?;
        Ok(Reply::done())
    }

    /// Indices of the entries the user marks, valid for `entries`.
    fn pick_entries(&self, title: &str, entries: &[profile::Entry]) -> anyhow::Result<Vec<usize>> {
        let labels: Vec<String> = entries.iter().map(ToString::to_string).collect();
        let picked = self.deps.prompter.multi_select(title, &labels)?;
        Ok(picked.into_iter().filter(|&i| i < entries.len()).collect())
    }
}
