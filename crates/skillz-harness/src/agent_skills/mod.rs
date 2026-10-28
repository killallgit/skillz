//! The open Agent Skills format (agentskills.io/specification), shared by every harness: a skill
//! is a directory holding `SKILL.md` (YAML frontmatter, then Markdown) and optional `scripts/`,
//! `references/` and `assets/`. Harnesses extend the frontmatter with keys of their own; those
//! are kept in `Frontmatter::extra` rather than dropped.

mod doctor;
mod frontmatter;
mod layout;
mod validate;

pub use doctor::{Diagnostic, Instance, Readers, Root, Stray, diagnose, roots};
pub use frontmatter::{Frontmatter, parse, skill_name};
pub use layout::{Layout, layout};
pub use validate::{Violation, validate};

/// Frontmatter names of every skill (a directory holding `SKILL.md`) under `root`, recursively,
/// sorted and without repeats: the names `npx skills --skill` matches.
pub fn skill_names_under(root: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let mut names: Vec<String> = skills_under(root)?
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// Each skill under `root`: its frontmatter name and its directory. `skill_names_under` is its names.
pub fn skills_under(root: &std::path::Path) -> anyhow::Result<Vec<(String, std::path::PathBuf)>> {
    let mut skills = Vec::new();
    crate::fswalk::walk(root, &mut |path, is_dir| {
        if is_dir && let Some(name) = skill_name(path) {
            skills.push((name, path.to_path_buf()));
        }
    })?;
    Ok(skills)
}

/// The file that makes a directory a skill.
pub const SKILL_FILE: &str = "SKILL.md";

/// The user and repo directory, relative to a home or project, where tools that follow the
/// open standard look for skills and where `npx skills` installs them.
pub const SHARED_DIR: &str = ".agents/skills";
