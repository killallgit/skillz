pub mod commands;
mod discover;
mod entry;
mod local;
mod ownership;
mod removal;

#[cfg(test)]
mod testutil;

use std::path::PathBuf;

use crate::Spec;
use crate::action::Action;
use crate::env::Env;
use crate::model::{Agent, Installed, Scope};

pub use discover::discover;

/// Directories `agent` loads loose skills from at `scope`. Codex also reads the shared
/// `.agents/skills` store that `npx skills` writes.
pub fn skill_dirs(env: &Env, agent: Agent, scope: Scope) -> Vec<PathBuf> {
    match (agent, scope) {
        (Agent::Claude, Scope::Global) => {
            vec![crate::agents::claude::dir(env).join("skills")]
        }
        (Agent::Claude, Scope::Project) => vec![env.project.join(".claude/skills")],
        (Agent::Codex, Scope::Global) => vec![
            crate::agents::codex::dir(env).join("skills"),
            commands::store_dir(env, scope),
        ],
        (Agent::Codex, Scope::Project) => vec![
            commands::store_dir(env, scope),
            env.project.join(".codex/skills"),
        ],
    }
}

/// The `npx skills add` that installs a skill spec for all its agents among `known`; `None` for
/// anything but a skill.
pub fn plan_install(spec: &Spec, known: &[Agent], scope: Scope, env: &Env) -> Option<Action> {
    let Spec::Skill { source, names, .. } = spec else {
        return None;
    };
    let agents: Vec<Agent> = spec
        .agents()
        .iter()
        .copied()
        .filter(|a| known.contains(a))
        .collect();
    let (program, args) = commands::add_command(source, names, &agents, scope);
    Some(Action::required(&program, args, &env.project))
}

/// The `npx skills update` for a loose skill called `name`, run from `cwd`.
pub fn plan_update(name: &str, scope: Scope, cwd: &std::path::Path) -> Action {
    let scope = match scope {
        Scope::Global => "-g",
        Scope::Project => "-p",
    };
    let args = ["-y", "skills", "update", name, scope, "-y"]
        .map(String::from)
        .to_vec();
    Action::required("npx", args, cwd)
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
    use std::path::Path;

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

    #[test]
    fn a_skill_installs_for_the_known_agents_it_names_and_nothing_else_does() {
        let skill = Spec::Skill {
            source: "o/r".into(),
            names: vec!["tdd".into()],
            agents: vec![Agent::Codex],
        };
        let hook = Spec::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "true".into(),
            agents: vec![],
        };

        let step = plan_install(
            &skill,
            &[Agent::Claude, Agent::Codex],
            Scope::Global,
            &env(),
        );

        let Some(Action::Run { args, .. }) = step else {
            panic!("a skill is installed by a command");
        };
        assert!(args.windows(2).any(|w| w == ["-a", "codex"]));
        assert!(!args.windows(2).any(|w| w == ["-a", "claude-code"]));
        assert!(plan_install(&hook, &[Agent::Claude], Scope::Global, &env()).is_none());
    }

    #[test]
    fn updating_a_loose_skill_runs_npx_with_the_scope_flag() {
        let step = plan_update("tdd", Scope::Project, Path::new("/p"));

        let Action::Run { program, args, .. } = step else {
            panic!("an update is a command");
        };
        assert_eq!(program, "npx");
        assert_eq!(args, ["-y", "skills", "update", "tdd", "-p", "-y"]);
    }
}
