//! What each skill and plugin costs in context: tokens loaded into every session.
//!
//! Each adapter reports `(key, tokens)` rows (`AgentAdapter::context_costs`), keyed
//! `skill:<name>` or `plugin:<name>` (no `@marketplace`); this module merges them per agent.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::agents::AgentAdapter;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Kind};
use crate::runner::CommandRunner;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Costs(HashMap<(Agent, String), u32>);

impl Costs {
    /// Always-on tokens of the item for the agents it serves, summed; `None` when no agent
    /// knows its cost.
    pub fn tokens(&self, item: &Augmentation) -> Option<u32> {
        let key = match item.kind {
            Kind::Skill => format!("skill:{}", item.name),
            Kind::Plugin => format!("plugin:{}", item.name.split('@').next()?),
            Kind::Marketplace | Kind::Hook => return None,
        };
        let found: Vec<u32> = item
            .agents
            .iter()
            .filter_map(|&a| self.0.get(&(a, key.clone())).copied())
            .collect();
        (!found.is_empty()).then(|| found.iter().sum())
    }
}

/// Every adapter's rows. An adapter that cannot report (tool missing, bad output) is skipped.
pub fn read(adapters: &[Box<dyn AgentAdapter>], env: &Env, runner: &dyn CommandRunner) -> Costs {
    let mut costs = Costs::default();
    for a in adapters {
        for (key, tokens) in a.context_costs(env, runner).unwrap_or_default() {
            *costs.0.entry((a.agent(), key)).or_default() += tokens;
        }
    }
    costs
}

/// Tokens a skill's listing takes: its frontmatter (name and description) at ~3.5 chars per
/// token, calibrated against Claude's own `/context` numbers.
// shortcut: a chars-based estimate, replace with the agent's own count when it reports one.
pub fn estimate_skill(skill_md: &Path) -> Option<u32> {
    let text = fs::read_to_string(skill_md).ok()?;
    let front = text.strip_prefix("---\n")?.split("\n---").next()?;
    Some((front.len() as u32 * 2).div_ceil(7))
}

/// Loose skills in `dirs`, estimated: `skill:<dir name>` for each subdir with a `SKILL.md`.
pub fn estimate_dirs(dirs: &[std::path::PathBuf]) -> Vec<(String, u32)> {
    dirs.iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            Some((
                format!("skill:{name}"),
                estimate_skill(&e.path().join("SKILL.md"))?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installer, Scope};

    fn item(kind: Kind, name: &str, agents: &[Agent]) -> Augmentation {
        Augmentation {
            id: name.into(),
            name: name.into(),
            kind,
            agents: agents.to_vec(),
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn tokens_sum_over_the_items_agents_and_ignore_the_marketplace() {
        let costs = Costs(
            [
                ((Agent::Claude, "skill:tdd".to_string()), 50),
                ((Agent::Codex, "skill:tdd".to_string()), 40),
                ((Agent::Claude, "plugin:caveman".to_string()), 1800),
            ]
            .into(),
        );
        let both = item(Kind::Skill, "tdd", &[Agent::Claude, Agent::Codex]);
        assert_eq!(costs.tokens(&both), Some(90));
        assert_eq!(
            costs.tokens(&item(Kind::Plugin, "caveman@caveman", &[Agent::Claude])),
            Some(1800)
        );
        assert_eq!(
            costs.tokens(&item(Kind::Skill, "tdd", &[Agent::Cursor])),
            None
        );
        assert_eq!(
            costs.tokens(&item(Kind::Hook, "tdd", &[Agent::Claude])),
            None
        );
    }

    #[test]
    fn estimate_reads_only_the_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("SKILL.md");
        fs::write(
            &md,
            format!(
                "---\nname: a\ndescription: {}\n---\n{}",
                "x".repeat(60),
                "body".repeat(500)
            ),
        )
        .unwrap();
        let tokens = estimate_skill(&md).unwrap();
        assert!((15..25).contains(&tokens), "{tokens}");
        fs::write(&md, "no frontmatter").unwrap();
        assert_eq!(estimate_skill(&md), None);
        assert_eq!(estimate_skill(&dir.path().join("missing")), None);
    }

    #[test]
    fn estimate_dirs_keys_by_directory_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("tdd")).unwrap();
        fs::write(dir.path().join("tdd/SKILL.md"), "---\nname: tdd\n---\n").unwrap();
        fs::create_dir_all(dir.path().join("empty")).unwrap();
        let rows = estimate_dirs(&[dir.path().to_path_buf(), dir.path().join("none")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "skill:tdd");
    }
}
