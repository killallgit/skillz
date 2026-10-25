//! The open Agent Skills format (agentskills.io/specification), shared by every harness: a skill
//! is a directory holding `SKILL.md` (YAML frontmatter, then Markdown) and optional `scripts/`,
//! `references/` and `assets/`. Harnesses extend the frontmatter with keys of their own; those
//! are kept in `Frontmatter::extra` rather than dropped.

mod frontmatter;
mod layout;
mod validate;

pub use frontmatter::{Frontmatter, parse, skill_name};
pub use layout::{Layout, layout};
pub use validate::{Violation, validate};

/// The file that makes a directory a skill.
pub const SKILL_FILE: &str = "SKILL.md";

/// The user and repo directory, relative to a home or project, where tools that follow the
/// open standard look for skills and where `npx skills` installs them.
pub const SHARED_DIR: &str = ".agents/skills";
