pub mod commands;
mod discover;
mod entry;
mod local;
mod ownership;
mod removal;

#[cfg(test)]
mod testutil;

use std::path::PathBuf;

use crate::harness::env::Env;
use crate::harness::model::{Agent, Installed, Scope};

pub use discover::discover;

/// Directories `agent` loads loose skills from at `scope`. Codex also reads the shared
/// `.agents/skills` store that `npx skills` writes.
pub fn skill_dirs(env: &Env, agent: Agent, scope: Scope) -> Vec<PathBuf> {
    match (agent, scope) {
        (Agent::Claude, Scope::Global) => {
            vec![crate::harness::agents::claude::dir(env).join("skills")]
        }
        (Agent::Claude, Scope::Project) => vec![env.project.join(".claude/skills")],
        (Agent::Codex, Scope::Global) => vec![
            crate::harness::agents::codex::dir(env).join("skills"),
            commands::store_dir(env, scope),
        ],
        (Agent::Codex, Scope::Project) => vec![
            commands::store_dir(env, scope),
            env.project.join(".codex/skills"),
        ],
    }
}

/// Every loose skill the `agents` see across `scopes`, one item per skill with the agents that
/// see it. One discovery over all scopes, so a project link into a global skill is its own item.
pub fn installed(env: &Env, agents: &[Agent], scopes: &[Scope]) -> anyhow::Result<Vec<Installed>> {
    let dirs: Vec<SkillDir> = scopes
        .iter()
        .flat_map(|&scope| {
            agents.iter().flat_map(move |&agent| {
                skill_dirs(env, agent, scope)
                    .into_iter()
                    .map(move |path| SkillDir { agent, scope, path })
            })
        })
        .collect();
    discover(env, &dirs)
}

/// A directory an agent loads skills from (each skill is a subdir with `SKILL.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub agent: Agent,
    pub scope: Scope,
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            home: "/h".into(),
            project: "/p".into(),
            claude_config: None,
            codex_home: None,
        }
    }

    #[test]
    fn each_agent_reads_its_own_dirs_and_codex_also_the_shared_store() {
        let env = env();
        let dirs = |agent, scope| skill_dirs(&env, agent, scope);

        assert_eq!(
            dirs(Agent::Claude, Scope::Global),
            [PathBuf::from("/h/.claude/skills")]
        );
        assert_eq!(
            dirs(Agent::Claude, Scope::Project),
            [PathBuf::from("/p/.claude/skills")]
        );
        assert_eq!(
            dirs(Agent::Codex, Scope::Global),
            [
                PathBuf::from("/h/.codex/skills"),
                PathBuf::from("/h/.agents/skills")
            ]
        );
        assert_eq!(
            dirs(Agent::Codex, Scope::Project),
            [
                PathBuf::from("/p/.agents/skills"),
                PathBuf::from("/p/.codex/skills")
            ]
        );
    }
}
