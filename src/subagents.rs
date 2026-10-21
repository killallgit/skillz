//! Subagent definitions: one file per agent in the agent's `agents/` dir (`.md` with
//! frontmatter for Claude, `.toml` for Codex). They have no installer of their own, so skillz
//! copies them from a source repo or directory and notes where each came from in
//! `.skillz-sources.json` next to them, which is how a profile snapshot can name the source.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::install::{clone_command, unique_name};
use crate::model::{Agent, Augmentation, Installer, Kind, Scope};
use crate::profile::Entry;
use crate::runner::CommandRunner;

const MANIFEST: &str = ".skillz-sources.json";

/// The agent's own name for a definition: `name:` in markdown frontmatter, `name =` in toml.
pub fn definition_name(text: &str, ext: &str) -> Option<String> {
    match ext {
        "toml" => text
            .parse::<toml_edit::DocumentMut>()
            .ok()?
            .get("name")?
            .as_str()
            .map(String::from),
        _ => {
            let mut lines = text.lines();
            if lines.next()?.trim_end() != "---" {
                return None;
            }
            lines
                .take_while(|l| l.trim_end() != "---")
                .find_map(|l| l.strip_prefix("name:"))
                .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
                .filter(|n| !n.is_empty())
        }
    }
}

/// `Frontend Developer` and `frontend-developer` are the same name.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn manifest(dir: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(dir.join(MANIFEST))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The definitions in `dir`, each removable by deleting its file (and its manifest line).
pub fn discover(dir: &Path, ext: &str, agent: Agent, scope: Scope) -> Vec<Augmentation> {
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    };
    let sources = manifest(dir);
    let mut files: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == ext))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let file = path.file_name()?.to_str()?.to_string();
            let stem = path.file_stem()?.to_str()?.to_string();
            let name = fs::read_to_string(&path)
                .ok()
                .and_then(|t| definition_name(&t, ext))
                .unwrap_or(stem);
            let source = sources.get(&file).cloned();
            let mut removal: Vec<Action> = RemovalTarget::new(dir, [&file])
                .map(Action::RemovePath)
                .into_iter()
                .collect();
            removal.push(Action::Edit {
                file: dir.join(MANIFEST),
                edit: Edit::JsonRemoveKey {
                    path: vec![file.clone()],
                },
            });
            Some(Augmentation {
                id: format!("subagent:{agent}:{scope}:{file}"),
                name,
                kind: Kind::Subagent,
                agents: vec![agent],
                scope,
                group: source.clone().unwrap_or_else(|| "local".into()),
                source,
                installer: Installer::Local,
                children: vec![format!("file: {file}")],
                removal,
            })
        })
        .collect()
}

/// The step that copies a profile's subagents into `to`. `None` for other kinds of entry.
pub fn provision(entry: &Entry, to: PathBuf, ext: &str, env: &Env) -> Option<Vec<Action>> {
    let Entry::Subagent { source, names, .. } = entry else {
        return None;
    };
    Some(vec![Action::CopyAgents {
        source: source.clone(),
        cwd: env.project.clone(),
        names: names.clone(),
        ext: ext.into(),
        to,
    }])
}

/// What `copy` needs.
pub struct Copy<'a> {
    pub source: &'a str,
    pub cwd: &'a Path,
    pub names: &'a [String],
    pub ext: &'a str,
    pub to: &'a Path,
}

/// Fetch `source` (a directory relative to `cwd`, else a git repo), copy the definitions called
/// `names` into `to` and record their source. Refuses to overwrite a file skillz did not put
/// there. Returns the files written.
pub fn copy(runner: &dyn CommandRunner, req: &Copy) -> anyhow::Result<Vec<PathBuf>> {
    let local = req.cwd.join(req.source);
    if local.is_dir() {
        return copy_from(&local, req);
    }
    let tmp = std::env::temp_dir().join(unique_name());
    let (program, args) = clone_command(req.source, &tmp);
    let cloned = runner.run_quiet(&program, &args, req.cwd).and_then(|out| {
        anyhow::ensure!(
            out.success,
            "cloning {} failed: {}",
            req.source,
            out.stderr.trim()
        );
        Ok(())
    });
    let result = cloned.and_then(|()| copy_from(&tmp, req));
    let _ = fs::remove_dir_all(&tmp);
    result
}

fn copy_from(root: &Path, req: &Copy) -> anyhow::Result<Vec<PathBuf>> {
    let mut found = vec![];
    walk(root, req.ext, &mut found)?;
    found.sort();
    let mut chosen = vec![];
    for wanted in req.names {
        let want = slug(wanted);
        let hit = found.iter().find(|(path, name)| {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            slug(stem) == want || name.as_deref().is_some_and(|n| slug(n) == want)
        });
        let Some((path, _)) = hit else {
            anyhow::bail!("no subagent called {wanted} in {}", req.source);
        };
        chosen.push(path.clone());
    }

    fs::create_dir_all(req.to)?;
    let mut sources = manifest(req.to);
    let mut written = vec![];
    for path in chosen {
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("file name")?;
        let dest = req.to.join(file);
        anyhow::ensure!(
            !dest.exists() || sources.contains_key(file),
            "{} exists and was not installed by skillz",
            dest.display()
        );
        fs::copy(&path, &dest).with_context(|| format!("copying {}", path.display()))?;
        sources.insert(file.to_string(), req.source.to_string());
        written.push(dest);
    }
    fs::write(
        req.to.join(MANIFEST),
        serde_json::to_string_pretty(&sources)?,
    )?;
    Ok(written)
}

/// Every `ext` file under `dir` with its definition name, skipping VCS and build dirs and
/// the readmes and docs repos keep beside them.
fn walk(dir: &Path, ext: &str, out: &mut Vec<(PathBuf, Option<String>)>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let skip = matches!(
            entry.file_name().to_str(),
            Some(".git" | "node_modules" | "dist" | "build" | "target")
        );
        let kind = entry.file_type()?;
        if kind.is_dir() && !skip {
            walk(&path, ext, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == ext) {
            let text = fs::read_to_string(&path).unwrap_or_default();
            let name = definition_name(&text, ext);
            // Only files that declare a name are definitions; this leaves READMEs out.
            if name.is_some() {
                out.push((path, name));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{NoCommands, Output};

    fn agent_file(dir: &Path, rel: &str, name: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: d\n---\nbody\n"),
        )
        .unwrap();
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_come_from_frontmatter_or_toml() {
        assert_eq!(
            definition_name("---\nname: \"Frontend Developer\"\n---\n", "md").as_deref(),
            Some("Frontend Developer")
        );
        assert_eq!(definition_name("# readme\nname: x\n", "md"), None);
        assert_eq!(definition_name("---\ndescription: d\n---\n", "md"), None);
        assert_eq!(
            definition_name("name = \"cx\"\n", "toml").as_deref(),
            Some("cx")
        );
        assert_eq!(definition_name("not toml [", "toml"), None);
        assert_eq!(slug("Frontend  Developer!"), "frontend-developer");
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
        let req = Copy {
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
        let req = Copy {
            source: &folder,
            ..req
        };
        let written = copy(&NoCommands, &req).unwrap();
        assert_eq!(written.len(), 2);
        assert!(to.path().join("engineering-frontend-developer.md").exists());
        assert!(!to.path().join("README.md").exists());
        let sources = manifest(to.path());
        assert_eq!(sources["ui-designer.md"], folder);

        let found = discover(to.path(), "md", Agent::Claude, Scope::Project);
        let mut got: Vec<(&str, Option<&str>)> = found
            .iter()
            .map(|a| (a.name.as_str(), a.source.as_deref()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("Frontend Developer", Some(folder.as_str())),
                ("UI Designer", Some(folder.as_str()))
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
                &Copy {
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
            &Copy {
                source: "o/r",
                cwd: cwd.path(),
                names: &wanted,
                ext: "md",
                to: to.path(),
            },
        )
        .unwrap();
        assert_eq!(written, [to.path().join("helper.md")]);
        assert_eq!(manifest(to.path())["helper.md"], "o/r");

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
            &Copy {
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
    fn removal_deletes_the_file_and_its_manifest_line() {
        let to = tempfile::tempdir().unwrap();
        agent_file(to.path(), "a.md", "A");
        fs::write(to.path().join(MANIFEST), r#"{"a.md": "o/r"}"#).unwrap();
        let item = discover(to.path(), "md", Agent::Claude, Scope::Global).remove(0);
        crate::action::execute(&item.removal, &NoCommands).unwrap();
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
                Path::new("/definitely/not/here"),
                "md",
                Agent::Claude,
                Scope::Global
            )
            .is_empty()
        );
        let to = tempfile::tempdir().unwrap();
        fs::write(to.path().join("x.txt"), "x").unwrap();
        assert!(discover(to.path(), "md", Agent::Claude, Scope::Global).is_empty());
    }
}
