use std::fs;
use std::path::{Path, PathBuf};

use super::SkillDir;
use crate::model::Agent;

/// One skill entry as seen from one agent dir.
#[derive(Debug, Clone)]
pub(super) struct Entry {
    pub agent: Agent,
    pub name: String,
    /// The entry inside the agent dir (a symlink or a real dir).
    pub link: PathBuf,
    /// Resolved skill dir; the link itself when the link is dangling.
    pub canonical: PathBuf,
}

pub(super) fn entries(dir: &SkillDir) -> anyhow::Result<Vec<Entry>> {
    // Canonical parent makes a real (non-symlinked) skill's link equal its canonical path.
    let base = canonical_or(&dir.path);
    let mut out: Vec<Entry> = crate::read_optional::read_dir(&base)?
        .into_iter()
        .filter_map(|e| entry(dir.agent, base.join(e.file_name())))
        .collect();
    out.sort_by(|a, b| a.link.cmp(&b.link));
    Ok(out)
}

fn entry(agent: Agent, link: PathBuf) -> Option<Entry> {
    let name = link.file_name()?.to_str()?.to_owned();
    if name.starts_with('.') {
        return None;
    }
    let canonical = if is_broken_link(&link) {
        link.clone()
    } else if link.join("SKILL.md").is_file() {
        fs::canonicalize(&link).ok()?
    } else {
        return None;
    };
    Some(Entry {
        agent,
        name,
        link,
        canonical,
    })
}

fn is_broken_link(p: &Path) -> bool {
    p.symlink_metadata().is_ok_and(|m| m.is_symlink()) && fs::metadata(p).is_err()
}

pub(super) fn canonical_or(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::super::discover;
    use super::super::testutil::*;
    use super::*;

    #[test]
    fn broken_link_is_reported_and_removes_only_the_link() {
        let f = fixture();
        let at = f.env.home.join(".claude/skills/gone");
        link(&f.env.home.join("nowhere"), &at);

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
        assert_eq!(augs[0].name, "gone");
        assert_eq!(augs[0].id, format!("skill:{}", at.display()));
        assert_eq!(files(&augs[0].removal), vec![rm(at)]);
    }

    #[test]
    fn skips_hidden_entries_files_and_dirs_without_skill_md() {
        let f = fixture();
        let codex = f.env.home.join(".codex/skills");
        skill(&codex.join(".system/builtin"));
        skill(&codex.join(".hidden"));
        fs::create_dir_all(codex.join("empty")).unwrap();
        fs::write(codex.join("README.md"), "x").unwrap();

        assert!(discover(&f.env, &global_dirs(&f.env)).unwrap().is_empty());
    }
}
