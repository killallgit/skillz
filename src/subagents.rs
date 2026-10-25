//! Subagent definitions: one file per agent in the agent's `agents/` dir (`.md` with
//! frontmatter for Claude, `.toml` for Codex): discovering them with their removal plans, and
//! the profile step that copies them (run by `subagent_store::copy`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::harness::Spec;
use crate::harness::action::{Action, Edit, RemovalTarget};
use crate::model::{Agent, Installed, Installer, Kind, Origin, Scope};
use crate::subagent_store::{MANIFEST, Manifest, definition_name};

/// Where an agent keeps its subagent definitions and what extension they carry.
pub struct AgentDir<'a> {
    pub path: &'a Path,
    pub ext: &'a str,
}

impl<'a> AgentDir<'a> {
    pub fn new(path: &'a Path, ext: &'a str) -> Self {
        AgentDir { path, ext }
    }
}

/// The definitions in `dir`, each removable by deleting its file (and its manifest line).
pub fn discover(dir: &AgentDir, agent: Agent, scope: Scope) -> anyhow::Result<Vec<Installed>> {
    let scan = Scan {
        dir,
        manifest: Manifest::read(dir.path)?,
        agent,
        scope,
    };
    definition_files(dir)?
        .iter()
        .map(|path| scan.installed_item(path))
        .collect::<anyhow::Result<Vec<_>>>()
        .map(|items| items.into_iter().flatten().collect())
}

/// The `ext` files directly in `dir`, sorted.
fn definition_files(dir: &AgentDir) -> anyhow::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = crate::harness::read_optional::read_dir(dir.path)?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == dir.ext))
        .collect();
    files.sort();
    Ok(files)
}

struct Scan<'a> {
    dir: &'a AgentDir<'a>,
    manifest: Manifest,
    agent: Agent,
    scope: Scope,
}

impl Scan<'_> {
    fn installed_item(&self, path: &Path) -> anyhow::Result<Option<Installed>> {
        let Some(file) = path.file_name().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        let text = fs::read_to_string(path)?;
        let name = definition_name(&text, self.dir.ext).unwrap_or_else(|| stem.to_string());
        let source = self.manifest.get(file).map(String::from);
        let origin = source
            .as_ref()
            .map_or_else(|| self.origin(), |_| Origin::default());
        Ok(Some(Installed {
            details: crate::model::Details::Subagent {
                file: path.to_path_buf(),
            },
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("subagent:{}:{}:{file}", self.agent, self.scope),
            name,
            kind: Kind::Subagent,
            agents: vec![self.agent],
            scope: self.scope,
            group: source.clone().unwrap_or_else(|| "local".into()),
            source,
            installer: Installer::Local,
            origin,
            children: vec![format!("file: {file}")],
            removal: self.removal(file),
        }))
    }

    /// A definition nothing recorded can still be copied again from the dir it sits in.
    fn origin(&self) -> Origin {
        Origin {
            link: Some(self.dir.path.to_string_lossy().into_owned()),
            version: None,
        }
    }

    fn removal(&self, file: &str) -> Vec<Action> {
        let mut steps: Vec<Action> = RemovalTarget::new(self.dir.path, [file])
            .map(Action::RemovePath)
            .into_iter()
            .collect();
        steps.push(Action::Edit {
            file: self.dir.path.join(MANIFEST),
            edit: Edit::JsonRemoveKey {
                path: vec![file.to_string()],
            },
        });
        steps
    }
}

/// The step that copies a profile's subagents into `to`. `None` for other kinds of entry.
pub fn provision(entry: &Spec, to: &AgentDir, env: &Env) -> Option<Vec<Action>> {
    let Spec::Subagent { source, names, .. } = entry else {
        return None;
    };
    Some(vec![Action::CopyAgents {
        source: source.clone(),
        cwd: env.project.clone(),
        names: names.clone(),
        ext: to.ext.into(),
        to: to.path.to_path_buf(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::runner::{CommandRunner, NoCommands, Output};
    use crate::subagent_store::{CopyRequest, copy};

    fn agent_file(dir: &Path, rel: &str, name: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: d\n---\nbody\n"),
        )
        .unwrap();
    }

    fn md_dir(path: &Path) -> AgentDir<'_> {
        AgentDir { path, ext: "md" }
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_hand_made_definition_can_be_copied_again_from_its_dir_and_a_recorded_one_is_not_redirected()
     {
        let dir = tempfile::tempdir().unwrap();
        agent_file(dir.path(), "mine.md", "Mine");
        agent_file(dir.path(), "copied.md", "Copied");
        fs::write(dir.path().join(MANIFEST), r#"{"copied.md":"o/r"}"#).unwrap();

        let found = discover(&md_dir(dir.path()), Agent::Claude, Scope::Project).unwrap();

        let link = |name: &str| {
            let item = found.iter().find(|a| a.name == name).unwrap();
            item.origin.link.clone()
        };
        assert_eq!(link("Mine"), Some(dir.path().to_string_lossy().into()));
        assert_eq!(link("Copied"), None);
    }

    #[test]
    fn copying_matches_by_file_stem_or_agent_name_and_notes_the_source() {
        let (src, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        agent_file(
            src.path(),
            "engineering/engineering-frontend-developer.md",
            "Frontend Developer",
        );
        agent_file(src.path(), "design/ui-designer.md", "UI Designer");
        fs::write(src.path().join("README.md"), "# not an agent\n").unwrap();
        let wanted = names(&["frontend-developer", "ui-designer"]);
        let req = CopyRequest {
            source: "agents-repo",
            cwd: src.path().parent().unwrap(),
            names: &wanted,
            ext: "md",
            to: to.path(),
        };
        // `source` is resolved against cwd, so point it at the tempdir by its folder name.
        let folder = src
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let req = CopyRequest {
            source: &folder,
            ..req
        };
        let written = copy(&NoCommands, &req).unwrap();
        assert_eq!(written.len(), 2);
        assert!(to.path().join("engineering-frontend-developer.md").exists());
        assert!(!to.path().join("README.md").exists());
        let sources = Manifest::read(to.path()).unwrap();
        let canonical_source = fs::canonicalize(src.path()).unwrap();
        assert_eq!(sources.get("ui-designer.md"), canonical_source.to_str());

        let found = discover(&md_dir(to.path()), Agent::Claude, Scope::Project).unwrap();
        let mut got: Vec<(&str, Option<&str>)> = found
            .iter()
            .map(|a| (a.name.as_str(), a.source.as_deref()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("Frontend Developer", canonical_source.to_str()),
                ("UI Designer", canonical_source.to_str())
            ]
        );
    }

    #[test]
    fn a_missing_name_or_a_foreign_file_is_an_error_and_copies_nothing() {
        let (src, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        agent_file(src.path(), "a.md", "A");
        let folder = src
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let cwd = src.path().parent().unwrap();
        let try_copy = |wanted: &[&str]| {
            let wanted = names(wanted);
            copy(
                &NoCommands,
                &CopyRequest {
                    source: &folder,
                    cwd,
                    names: &wanted,
                    ext: "md",
                    to: to.path(),
                },
            )
        };
        assert!(try_copy(&["a", "missing"]).is_err());
        assert!(
            !to.path().join("a.md").exists(),
            "nothing is copied when a name is missing"
        );

        fs::write(to.path().join("a.md"), "mine").unwrap();
        assert!(
            try_copy(&["a"]).is_err(),
            "hand-made file is not overwritten"
        );
        assert_eq!(fs::read_to_string(to.path().join("a.md")).unwrap(), "mine");

        fs::remove_file(to.path().join("a.md")).unwrap();
        try_copy(&["a"]).unwrap();
        try_copy(&["a"]).unwrap();
    }

    #[test]
    fn a_source_that_is_not_a_directory_is_cloned_and_the_clone_removed() {
        struct Clone;
        impl CommandRunner for Clone {
            fn run(&self, program: &str, args: &[String], _: &Path) -> anyhow::Result<Output> {
                assert_eq!(program, "git");
                let dest = Path::new(args.last().unwrap());
                agent_file(dest, "x/helper.md", "Helper");
                Ok(Output {
                    success: true,
                    stderr: String::new(),
                })
            }
        }
        let to = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let wanted = names(&["helper"]);
        let written = copy(
            &Clone,
            &CopyRequest {
                source: "o/r",
                cwd: cwd.path(),
                names: &wanted,
                ext: "md",
                to: to.path(),
            },
        )
        .unwrap();
        assert_eq!(written, [to.path().join("helper.md")]);
        assert_eq!(
            Manifest::read(to.path()).unwrap().get("helper.md"),
            Some("o/r")
        );

        struct Fails;
        impl CommandRunner for Fails {
            fn run(&self, _: &str, _: &[String], _: &Path) -> anyhow::Result<Output> {
                Ok(Output {
                    success: false,
                    stderr: "nope".into(),
                })
            }
        }
        let err = copy(
            &Fails,
            &CopyRequest {
                source: "o/r",
                cwd: cwd.path(),
                names: &wanted,
                ext: "md",
                to: to.path(),
            },
        );
        assert!(err.is_err());
    }

    #[test]
    fn a_corrupt_manifest_is_an_error_and_is_left_alone() {
        let (src, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        agent_file(src.path(), "helper.md", "helper");
        fs::write(to.path().join(MANIFEST), "{nope").unwrap();
        let wanted = names(&["helper"]);
        let req = CopyRequest {
            source: src.path().to_str().unwrap(),
            cwd: src.path(),
            names: &wanted,
            ext: "md",
            to: to.path(),
        };

        let copied = copy(&NoCommands, &req);
        let found = discover(&md_dir(to.path()), Agent::Claude, Scope::Global);

        // neither copying nor discovering may treat the broken manifest as empty
        assert!(copied.is_err() && found.is_err());
        assert_eq!(
            fs::read_to_string(to.path().join(MANIFEST)).unwrap(),
            "{nope"
        );
        assert!(!to.path().join("helper.md").exists());
    }

    #[test]
    fn removal_deletes_the_file_and_its_manifest_line() {
        let to = tempfile::tempdir().unwrap();
        agent_file(to.path(), "a.md", "A");
        fs::write(to.path().join(MANIFEST), r#"{"a.md": "o/r"}"#).unwrap();
        let item = discover(&md_dir(to.path()), Agent::Claude, Scope::Global)
            .unwrap()
            .remove(0);
        crate::harness::action::execute(&item.removal, &NoCommands).unwrap();
        assert!(!to.path().join("a.md").exists());
        assert!(
            !fs::read_to_string(to.path().join(MANIFEST))
                .unwrap()
                .contains("a.md")
        );
    }

    #[test]
    fn missing_directories_and_foreign_extensions_find_nothing() {
        assert!(
            discover(
                &md_dir(Path::new("/definitely/not/here")),
                Agent::Claude,
                Scope::Global
            )
            .unwrap()
            .is_empty()
        );
        let to = tempfile::tempdir().unwrap();
        fs::write(to.path().join("x.txt"), "x").unwrap();
        assert!(
            discover(&md_dir(to.path()), Agent::Claude, Scope::Global)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn a_relative_local_source_remains_updateable_from_another_project() {
        let (source, target, other) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let repository = source.path().join("repo");
        agent_file(&repository, "helper.md", "Helper");
        let names = names(&["Helper"]);
        copy(
            &NoCommands,
            &CopyRequest {
                source: "repo",
                cwd: source.path(),
                names: &names,
                ext: "md",
                to: target.path(),
            },
        )
        .unwrap();
        let item = discover(&md_dir(target.path()), Agent::Claude, Scope::Global)
            .unwrap()
            .remove(0);
        let canonical_repository = repository.canonicalize().unwrap();
        assert_eq!(item.source.as_deref(), canonical_repository.to_str());
        let changed = "---\nname: Helper\n---\nchanged body\n";
        fs::write(repository.join("helper.md"), changed).unwrap();

        let plan = crate::update::steps(&crate::agents::registry(), &item, other.path());
        crate::harness::action::execute(&plan, &NoCommands).unwrap();

        assert_eq!(
            fs::read_to_string(target.path().join("helper.md")).unwrap(),
            changed
        );
        assert_eq!(
            Manifest::read(target.path()).unwrap().get("helper.md"),
            canonical_repository.to_str()
        );
    }
}
