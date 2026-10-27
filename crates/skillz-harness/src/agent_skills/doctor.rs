//! What is wrong with the skills on disk, as typed diagnostics: names that collide, skill files
//! in places a tool will not look (or looks too hard), files that break the format and links that
//! lead nowhere. Reads the folders it is given and changes nothing.
//!
//! Which copy a tool keeps when names collide differs per tool and is not decided here: a
//! collision lists every copy, in the order the roots were given.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{Frontmatter, SKILL_FILE, Violation, parse, validate};
use crate::env::Env;
use crate::model::{Agent, Scope};

/// A folder skills live in, and the tools that read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub dir: PathBuf,
    pub scope: Scope,
    pub agents: Vec<Agent>,
}

/// The folders each tool loads skills from, at user level and in the project; one that several
/// tools read is one root.
pub fn roots(env: &Env) -> Vec<Root> {
    let mut roots: Vec<Root> = Vec::new();
    for scope in [Scope::Global, Scope::Project] {
        for agent in [Agent::Claude, Agent::Codex] {
            for dir in crate::npx::skill_dirs(env, agent, scope) {
                match roots.iter_mut().find(|r| r.dir == dir && r.scope == scope) {
                    Some(root) => root.agents.push(agent),
                    None => roots.push(Root {
                        dir,
                        scope,
                        agents: vec![agent],
                    }),
                }
            }
        }
    }
    roots
}

/// Who would meet a collision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readers {
    /// These tools, each of which reads every copy as a skill.
    Agents(Vec<Agent>),
    /// A tool that also loads nested and loose skill files (pi does), which sees every copy.
    Recursive,
}

/// Why a skill file sits where the tools do not expect one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stray {
    /// A `SKILL.md` below a folder that is not itself a skill: only tools that search deeper
    /// find it.
    Nested,
    /// Skill-like frontmatter in a file that is not called `SKILL.md`.
    WrongName,
    /// A skill-like file directly in the skills folder, outside any skill folder.
    Loose,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Copy {
    pub path: PathBuf,
    pub scope: Scope,
    pub stray: Option<Stray>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnostic {
    /// Several skill files answer to `name`; `copies` lists them all.
    Collision {
        name: String,
        readers: Readers,
        copies: Vec<Copy>,
    },
    Stray {
        path: PathBuf,
        name: Option<String>,
        scope: Scope,
        why: Stray,
    },
    /// A skill that breaks the Agent Skills format.
    Invalid {
        path: PathBuf,
        name: String,
        scope: Scope,
        violations: Vec<Violation>,
    },
    /// A link in a skills folder whose target is gone.
    Dangling {
        path: PathBuf,
        target: Option<PathBuf>,
        scope: Scope,
    },
}

/// A skill file found while scanning, real or stray.
struct Found {
    name: String,
    file: PathBuf,
    /// The file with every link followed, so two links to one file are one skill.
    real: PathBuf,
    root: usize,
    stray: Option<Stray>,
}

#[derive(Default)]
struct Scan {
    found: Vec<Found>,
    diagnostics: Vec<Diagnostic>,
    /// Files already reported, by real path: a link must not report its target twice.
    reported: HashSet<PathBuf>,
}

/// Every problem in the skills under `roots`: collisions first, then strays, format problems and
/// dangling links, each in the order found.
pub fn diagnose(roots: &[Root]) -> Vec<Diagnostic> {
    let mut scan = Scan::default();
    for (index, root) in roots.iter().enumerate() {
        scan.root(index, root);
    }
    let mut diagnostics = collisions(roots, &scan.found);
    diagnostics.extend(scan.diagnostics);
    diagnostics
}

impl Scan {
    fn root(&mut self, index: usize, root: &Root) {
        for (name, path) in entries(&root.dir) {
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = fs::metadata(&path) else {
                self.dangling(&path, root.scope);
                continue;
            };
            if meta.is_dir() {
                self.top_dir(index, root, (&name, &path));
            } else if is_markdown(&path) && skill_like(&path).is_some() {
                self.stray(index, root, &path, Stray::Loose);
            }
        }
    }

    fn dangling(&mut self, path: &Path, scope: Scope) {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            self.diagnostics.push(Diagnostic::Dangling {
                path: path.to_path_buf(),
                target: fs::read_link(path).ok(),
                scope,
            });
        }
    }

    /// A folder directly in a skills folder: a skill when it holds `SKILL.md`, else searched for
    /// skill files that ended up inside.
    fn top_dir(&mut self, index: usize, root: &Root, (name, dir): (&str, &Path)) {
        let Some(file) = exact_skill_file(dir) else {
            return self.nested(index, root, dir);
        };
        let text = fs::read_to_string(&file).unwrap_or_default();
        let front = parse(&text);
        let skill = front
            .as_ref()
            .and_then(|f| f.name.clone())
            .unwrap_or_else(|| name.to_string());
        let real = fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
        let first_time = self.reported.insert(real.clone());
        if first_time {
            let violations = match &front {
                Some(front) => validate(front, name),
                None => vec![Violation::MissingName, Violation::MissingDescription],
            };
            if !violations.is_empty() {
                self.diagnostics.push(Diagnostic::Invalid {
                    path: file.clone(),
                    name: skill.clone(),
                    scope: root.scope,
                    violations,
                });
            }
        }
        self.found.push(Found {
            name: skill,
            file,
            real,
            root: index,
            stray: None,
        });
    }

    /// Search below a folder that is not a skill, without following links, for skill files.
    fn nested(&mut self, index: usize, root: &Root, dir: &Path) {
        if let Some(file) = exact_skill_file(dir) {
            return self.stray(index, root, &file, Stray::Nested);
        }
        for (name, path) in entries(dir) {
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                self.nested(index, root, &path);
            } else if meta.is_file() && is_markdown(&path) && skill_like(&path).is_some() {
                self.stray(index, root, &path, Stray::WrongName);
            }
        }
    }

    fn stray(&mut self, index: usize, root: &Root, file: &Path, why: Stray) {
        let front = skill_like(file);
        let name = front.as_ref().and_then(|f| f.name.clone());
        let real = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        if self.reported.insert(real.clone()) {
            self.diagnostics.push(Diagnostic::Stray {
                path: file.to_path_buf(),
                name: name.clone(),
                scope: root.scope,
                why,
            });
        }
        // A file without a name of its own is called after the folder holding it.
        let name = name
            .or_else(|| parent_name(file))
            .unwrap_or_else(|| "?".into());
        self.found.push(Found {
            name,
            file: file.to_path_buf(),
            real,
            root: index,
            stray: Some(why),
        });
    }
}

/// Names that more than one skill file answers to: for each tool that reads the copies as
/// skills, and for tools that also load the strays.
fn collisions(roots: &[Root], found: &[Found]) -> Vec<Diagnostic> {
    type Key = (String, Vec<PathBuf>);
    let mut by_agents: BTreeMap<Key, (Vec<Agent>, Vec<usize>)> = BTreeMap::new();
    for agent in [Agent::Claude, Agent::Codex] {
        let readable = found
            .iter()
            .enumerate()
            .filter(|(_, f)| f.stray.is_none() && roots[f.root].agents.contains(&agent));
        for (name, copies) in grouped(readable) {
            let key = (
                name,
                copies.iter().map(|&i| found[i].real.clone()).collect(),
            );
            let entry = by_agents.entry(key).or_default();
            entry.0.push(agent);
            entry.1 = copies;
        }
    }
    let mut diagnostics: Vec<Diagnostic> = by_agents
        .into_iter()
        .map(|((name, _), (agents, copies))| Diagnostic::Collision {
            name,
            readers: Readers::Agents(agents),
            copies: copies_of(roots, found, &copies),
        })
        .collect();
    // Only the groups a stray takes part in are new: the others are said above.
    for (name, copies) in grouped(found.iter().enumerate()) {
        if copies.iter().any(|&i| found[i].stray.is_some()) {
            diagnostics.push(Diagnostic::Collision {
                name,
                readers: Readers::Recursive,
                copies: copies_of(roots, found, &copies),
            });
        }
    }
    diagnostics
}

/// The files by name that more than one distinct file answers to, as indexes in the order found.
fn grouped<'a>(files: impl Iterator<Item = (usize, &'a Found)>) -> Vec<(String, Vec<usize>)> {
    let mut seen: HashSet<&Path> = HashSet::new();
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (at, file) in files {
        if seen.insert(&file.real) {
            by_name.entry(&file.name).or_default().push(at);
        }
    }
    by_name
        .into_iter()
        .filter(|(_, copies)| copies.len() > 1)
        .map(|(name, copies)| (name.to_string(), copies))
        .collect()
}

fn copies_of(roots: &[Root], found: &[Found], at: &[usize]) -> Vec<Copy> {
    at.iter()
        .map(|&i| Copy {
            path: found[i].file.clone(),
            scope: roots[found[i].root].scope,
            stray: found[i].stray,
        })
        .collect()
}

/// The children of `dir` by name, sorted so reports are the same every time.
fn entries(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut entries: Vec<(String, PathBuf)> = read
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    entries.sort();
    entries
}

/// `dir/SKILL.md`, exactly so: a case-insensitive disk would answer to `skill.md` as well.
fn exact_skill_file(dir: &Path) -> Option<PathBuf> {
    entries(dir)
        .into_iter()
        .find(|(name, path)| name == SKILL_FILE && path.is_file())
        .map(|(_, path)| path)
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

/// The frontmatter of a file that names itself and describes itself, as a skill does.
fn skill_like(file: &Path) -> Option<Frontmatter> {
    let front = parse(&fs::read_to_string(file).ok()?)?;
    (front.name.is_some() && front.description.is_some()).then_some(front)
}

fn parent_name(file: &Path) -> Option<String> {
    Some(file.parent()?.file_name()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
