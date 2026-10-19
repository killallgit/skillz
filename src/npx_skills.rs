//! The npx `skills` tool (vercel-labs/skills): its skill store, its lock file and its `add`
//! command line. Discovery (`skills`), install and the Codex adapter all go through here.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::env::Env;
use crate::model::{Agent, Scope};

/// The canonical store agent skill dirs symlink into: `<root>/.agents/skills`.
pub fn store_dir(env: &Env, scope: Scope) -> PathBuf {
    env.root(scope).join(".agents/skills")
}

pub fn lock_path(env: &Env, scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => env.home.join(".agents/.skill-lock.json"),
        Scope::Project => env.project.join("skills-lock.json"),
    }
}

/// Skill name -> `source` from `{"skills": {"<name>": {"source": "org/repo"}}}`.
/// Missing or corrupt lock files read as empty so skills are still discovered.
pub fn read_lock(env: &Env, scope: Scope) -> BTreeMap<String, String> {
    let json: Value = fs::read_to_string(lock_path(env, scope))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    json.get("skills")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, v)| Some((name.clone(), v.get("source")?.as_str()?.to_owned())))
        .collect()
}

/// JSON key path of `name`'s entry in the lock file.
pub fn lock_key(name: &str) -> Vec<String> {
    vec!["skills".into(), name.into()]
}

pub fn agent_id(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude-code",
        Agent::Codex => "codex",
        Agent::Cursor => "cursor",
    }
}

/// `npx -y skills add <source> --skill <s>... -a <agent>... [-g] -y`.
pub fn add_command(
    source: &str,
    skills: &[String],
    agents: &[Agent],
    scope: Scope,
) -> (String, Vec<String>) {
    let mut args: Vec<String> = ["-y", "skills", "add", source].map(String::from).to_vec();
    for s in skills {
        args.extend(["--skill".into(), s.clone()]);
    }
    for a in agents {
        args.extend(["-a".into(), agent_id(*a).into()]);
    }
    if scope == Scope::Global {
        args.push("-g".into());
    }
    args.push("-y".into());
    ("npx".into(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn add_command_maps_agent_ids_and_global_flag() {
        let (prog, args) = add_command(
            "o/r",
            &s(&["a"]),
            &[Agent::Claude, Agent::Codex, Agent::Cursor],
            Scope::Global,
        );
        assert_eq!(prog, "npx");
        assert_eq!(
            args,
            s(&[
                "-y",
                "skills",
                "add",
                "o/r",
                "--skill",
                "a",
                "-a",
                "claude-code",
                "-a",
                "codex",
                "-a",
                "cursor",
                "-g",
                "-y"
            ])
        );
        let (_, project) = add_command("o/r", &s(&["a"]), &[Agent::Codex], Scope::Project);
        assert!(!project.contains(&"-g".to_string()));
    }

    #[test]
    fn read_lock_keeps_string_sources_only() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Env {
            home: tmp.path().join("h"),
            project: tmp.path().join("p"),
        };
        assert!(read_lock(&env, Scope::Project).is_empty());
        fs::create_dir_all(&env.project).unwrap();
        fs::write(
            lock_path(&env, Scope::Project),
            r#"{"skills":{"a":{"source":"o/r"},"b":{"source":1},"c":{}}}"#,
        )
        .unwrap();
        let lock = read_lock(&env, Scope::Project);
        assert_eq!(lock, BTreeMap::from([("a".into(), "o/r".into())]));
    }
}
