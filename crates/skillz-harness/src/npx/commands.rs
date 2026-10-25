//! The npx `skills` tool (vercel-labs/skills): its skill store, its lock file and its `add`
//! command line. Discovery (`skills`), install and the Codex adapter all go through here.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Context;
use serde_json::Value;

use crate::env::Env;
use crate::model::{Agent, Scope};
use crate::read_optional::read_optional;

/// The canonical store agent skill dirs symlink into: `<root>/.agents/skills`.
pub fn store_dir(env: &Env, scope: Scope) -> PathBuf {
    env.root(scope).join(crate::agent_skills::SHARED_DIR)
}

pub fn lock_path(env: &Env, scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => env.home.join(".agents/.skill-lock.json"),
        Scope::Project => env.project.join("skills-lock.json"),
    }
}

/// What the lock records about one skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    pub source: String,
    /// `sourceUrl`: where it can be fetched from again.
    pub link: Option<String>,
    /// `skillFolderHash` (project locks: `computedHash`): the content it was installed at.
    pub hash: Option<String>,
}

/// Skill name -> lock entry from `{"skills": {"<name>": {"source": "org/repo", ...}}}`.
/// A missing lock reads as empty; a corrupt one is an error so it is never skipped silently.
pub fn read_lock(env: &Env, scope: Scope) -> anyhow::Result<BTreeMap<String, LockEntry>> {
    let path = lock_path(env, scope);
    let Some(text) = read_optional(&path)? else {
        return Ok(BTreeMap::new());
    };
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("malformed skills lock {}", path.display()))?;
    Ok(json
        .get("skills")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, v)| Some((name.clone(), lock_entry(v)?)))
        .collect())
}

fn lock_entry(value: &Value) -> Option<LockEntry> {
    let text = |key: &str| value.get(key)?.as_str().map(str::to_owned);
    Some(LockEntry {
        source: text("source")?,
        link: text("sourceUrl"),
        hash: text("skillFolderHash").or_else(|| text("computedHash")),
    })
}

/// JSON key path of `name`'s entry in the lock file.
pub fn lock_key(name: &str) -> Vec<String> {
    vec!["skills".into(), name.into()]
}

/// `npx -y skills add <source> --skill <s>... -a <agent>... [-g] -y`.
pub fn add_command(
    source: &str,
    skills: &[String],
    agents: &[Agent],
    scope: Scope,
) -> (String, Vec<String>) {
    let mut args = skills_args(&["add", source]);
    for s in skills {
        args.extend(["--skill".into(), s.clone()]);
    }
    finish(args, agents, scope)
}

/// `npx -y skills remove <name> -a <agent>... [-g] -y`: the CLI's own uninstall, which also
/// drops the lock entry.
pub fn remove_command(name: &str, agents: &[Agent], scope: Scope) -> (String, Vec<String>) {
    finish(skills_args(&["remove", name]), agents, scope)
}

fn skills_args(subcommand: &[&str]) -> Vec<String> {
    ["-y", "skills"]
        .iter()
        .chain(subcommand)
        .map(|a| a.to_string())
        .collect()
}

fn finish(mut args: Vec<String>, agents: &[Agent], scope: Scope) -> (String, Vec<String>) {
    for a in agents {
        args.extend(["-a".into(), a.npx_id().into()]);
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
    use crate::test_support::s;
    use std::fs;

    #[test]
    fn remove_command_names_skill_agents_and_scope() {
        let (prog, args) = remove_command("tdd", &[Agent::Claude], Scope::Global);
        assert_eq!(prog, "npx");
        assert_eq!(
            args,
            s(&[
                "-y",
                "skills",
                "remove",
                "tdd",
                "-a",
                "claude-code",
                "-g",
                "-y"
            ])
        );
        let (_, project) = remove_command("tdd", &[Agent::Codex], Scope::Project);
        assert_eq!(
            project,
            s(&["-y", "skills", "remove", "tdd", "-a", "codex", "-y"])
        );
    }

    #[test]
    fn add_command_maps_agent_ids_and_global_flag() {
        let (prog, args) = add_command(
            "o/r",
            &s(&["a"]),
            &[Agent::Claude, Agent::Codex],
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
                "-g",
                "-y"
            ])
        );
        let (_, project) = add_command("o/r", &s(&["a"]), &[Agent::Codex], Scope::Project);
        assert!(!project.contains(&"-g".to_string()));
    }

    #[test]
    fn a_lock_entry_carries_its_link_and_content_hash() {
        let global = serde_json::json!({"source": "o/r", "sourceUrl": "https://h/o/r.git", "skillFolderHash": "abc"});
        let project = serde_json::json!({"source": "o/r", "computedHash": "def"});

        assert_eq!(
            lock_entry(&global),
            Some(LockEntry {
                source: "o/r".into(),
                link: Some("https://h/o/r.git".into()),
                hash: Some("abc".into())
            })
        );
        assert_eq!(lock_entry(&project).unwrap().hash.as_deref(), Some("def"));
    }

    #[test]
    fn read_lock_keeps_string_sources_only() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Env {
            home: tmp.path().join("h"),
            project: tmp.path().join("p"),
            claude_config: None,
            codex_home: None,
        };
        assert!(read_lock(&env, Scope::Project).unwrap().is_empty());
        fs::create_dir_all(&env.project).unwrap();
        fs::write(
            lock_path(&env, Scope::Project),
            r#"{"skills":{"a":{"source":"o/r"},"b":{"source":1},"c":{}}}"#,
        )
        .unwrap();
        let lock = read_lock(&env, Scope::Project).unwrap();
        assert_eq!(lock.keys().collect::<Vec<_>>(), ["a"]);
        assert_eq!(lock["a"].source, "o/r");
    }
}
