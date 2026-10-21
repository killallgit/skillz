//! Load a profile into a scope and take it out again. What a profile added is found by reading
//! the inventory before and after, and recorded in a ledger: unapplying removes exactly those
//! items, never something that was already there or that was installed by hand since.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::action::{self, Action, Report};
use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::model::{Augmentation, Scope};
use crate::profile::Profile;
use crate::provision::{self, Unsupported};
use crate::remove::{self, Outcome, Plan};
use crate::runner::CommandRunner;

/// One item a profile added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    pub name: String,
}

/// What a profile put into one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub profile: String,
    pub items: Vec<Owned>,
}

/// The records of every scope skillz has applied a profile to, in one json file.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    pub fn new(path: PathBuf) -> Self {
        Ledger { path }
    }

    /// `<skillz dir>/applied.json`.
    pub fn in_env(env: &Env) -> Self {
        Ledger::new(env.skillz_dir().join("applied.json"))
    }

    /// Which scope a record is about: the project's path, or `global`.
    pub fn key(env: &Env, scope: Scope) -> String {
        match scope {
            Scope::Global => "global".into(),
            Scope::Project => env.project.display().to_string(),
        }
    }

    fn read(&self) -> anyhow::Result<BTreeMap<String, Record>> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid {}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e).with_context(|| format!("reading {}", self.path.display())),
        }
    }

    pub fn get(&self, key: &str) -> anyhow::Result<Option<Record>> {
        Ok(self.read()?.remove(key))
    }

    /// Store `record` for `key`, or forget `key` when `record` is `None`.
    pub fn put(&self, key: &str, record: Option<Record>) -> anyhow::Result<()> {
        let mut all = self.read()?;
        match record {
            Some(r) => all.insert(key.into(), r),
            None => all.remove(key),
        };
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&all)?)
            .with_context(|| format!("writing {}", self.path.display()))
    }
}

/// What applying a profile did.
#[derive(Debug)]
pub struct Applied {
    pub report: Report,
    /// Items the profile added; items already there are not listed (and not owned).
    pub added: Vec<Owned>,
    pub unsupported: Vec<Unsupported>,
}

/// Run the profile's install steps in `scope` and record what appeared. A scope holds one
/// profile at a time. A failed step still records whatever it got installed first, so that
/// can be unapplied, then returns the error.
pub fn apply(
    adapters: &[Box<dyn AgentAdapter>],
    profile: &Profile,
    scope: Scope,
    env: &Env,
    ledger: &Ledger,
    runner: &dyn CommandRunner,
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Applied> {
    let key = Ledger::key(env, scope);
    if let Some(current) = ledger.get(&key)? {
        anyhow::bail!(
            "profile {} is already applied here; unapply it first",
            current.profile
        );
    }
    let provision = provision::plan(adapters, profile, scope, env)?;
    let before: HashSet<String> = agents::inventory(adapters, env, &[scope])
        .items
        .into_iter()
        .map(|i| i.id)
        .collect();
    let ran = action::execute_with(&provision.steps, runner, on_step);
    let added: Vec<Owned> = agents::inventory(adapters, env, &[scope])
        .items
        .into_iter()
        .filter(|i| !before.contains(&i.id))
        .map(|i| Owned {
            id: i.id,
            name: i.name,
        })
        .collect();
    if !added.is_empty() || ran.is_ok() {
        ledger.put(
            &key,
            Some(Record {
                profile: profile.name.clone(),
                items: added.clone(),
            }),
        )?;
    }
    Ok(Applied {
        report: ran?,
        added,
        unsupported: provision.unsupported,
    })
}

/// The installed items the ledger says `scope`'s profile added. Ones that have since gone are
/// not listed.
pub fn owned<'a>(
    ledger: &Ledger,
    env: &Env,
    scope: Scope,
    installed: &'a [Augmentation],
) -> anyhow::Result<(Option<String>, Vec<&'a Augmentation>)> {
    let Some(record) = ledger.get(&Ledger::key(env, scope))? else {
        return Ok((None, vec![]));
    };
    let ids: HashSet<&str> = record.items.iter().map(|o| o.id.as_str()).collect();
    let items = installed
        .iter()
        .filter(|i| ids.contains(i.id.as_str()))
        .collect();
    Ok((Some(record.profile), items))
}

/// Remove what `scope`'s profile added. The record is forgotten once everything is gone, and
/// trimmed to what is left when something stays. `None` when no profile is applied.
pub fn unapply(
    adapters: &[Box<dyn AgentAdapter>],
    scope: Scope,
    env: &Env,
    ledger: &Ledger,
    runner: &dyn CommandRunner,
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Option<Outcome>> {
    let installed = agents::inventory(adapters, env, &[scope]).items;
    let (profile, chosen) = owned(ledger, env, scope, &installed)?;
    let Some(profile) = profile else {
        return Ok(None);
    };
    let plan = Plan::new(&chosen, &[]);
    let outcome = remove::run(&plan, adapters, env, &[scope], runner, on_step)?;
    let left: Vec<Owned> = chosen
        .iter()
        .filter(|c| outcome.still_installed.contains(&c.name))
        .map(|c| Owned {
            id: c.id.clone(),
            name: c.name.clone(),
        })
        .collect();
    let record = (!left.is_empty()).then_some(Record {
        profile,
        items: left,
    });
    ledger.put(&Ledger::key(env, scope), record)?;
    Ok(Some(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::profile::Entry;
    use crate::runner::Output;
    use std::cell::RefCell;
    use std::path::Path;

    /// Installs a `tdd` skill into the project's npx store when asked to `skills add`, like
    /// the real tool would; fails every command when `fail` is set.
    struct NpxFake {
        project: PathBuf,
        calls: RefCell<usize>,
        fail: bool,
    }

    impl NpxFake {
        fn new(project: &Path) -> Self {
            NpxFake {
                project: project.into(),
                calls: RefCell::new(0),
                fail: false,
            }
        }
    }

    impl CommandRunner for NpxFake {
        fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
            *self.calls.borrow_mut() += 1;
            if !self.fail && args.iter().any(|a| a == "add") {
                let dir = self.project.join(".agents/skills/tdd");
                fs::create_dir_all(&dir)?;
                fs::write(
                    dir.join("SKILL.md"),
                    "---\nname: tdd\ndescription: d\n---\n",
                )?;
            }
            Ok(Output {
                success: !self.fail,
                stderr: "boom".into(),
            })
        }
    }

    struct Fx {
        _dirs: (tempfile::TempDir, tempfile::TempDir),
        env: Env,
        ledger: Ledger,
    }

    fn fx() -> Fx {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let env = Env::new(home.path().into(), project.path().into());
        Fx {
            ledger: Ledger::in_env(&env),
            env,
            _dirs: (home, project),
        }
    }

    fn profile() -> Profile {
        let mut p = Profile::new("web");
        p.entries = vec![Entry::Skill {
            source: "o/r".into(),
            names: vec!["tdd".into()],
            agents: vec![],
        }];
        p
    }

    fn apply_to(fx: &Fx, runner: &dyn CommandRunner) -> anyhow::Result<Applied> {
        apply(
            &registry(),
            &profile(),
            Scope::Project,
            &fx.env,
            &fx.ledger,
            runner,
            &mut |_| {},
        )
    }

    fn unapply_from(fx: &Fx, runner: &dyn CommandRunner) -> Option<Outcome> {
        unapply(
            &registry(),
            Scope::Project,
            &fx.env,
            &fx.ledger,
            runner,
            &mut |_| {},
        )
        .unwrap()
    }

    #[test]
    fn apply_records_what_appeared_and_unapply_removes_exactly_that() {
        let fx = fx();
        let by_hand = fx.env.project.join(".claude/skills/mine");
        fs::create_dir_all(&by_hand).unwrap();
        fs::write(
            by_hand.join("SKILL.md"),
            "---\nname: mine\ndescription: d\n---\n",
        )
        .unwrap();
        let runner = NpxFake::new(&fx.env.project);

        let applied = apply_to(&fx, &runner).unwrap();
        assert_eq!(applied.added.len(), 1);
        assert_eq!(applied.added[0].name, "tdd");
        let record = fx
            .ledger
            .get(&Ledger::key(&fx.env, Scope::Project))
            .unwrap();
        assert_eq!(record.unwrap().profile, "web");

        let out = unapply_from(&fx, &runner).unwrap();
        assert!(out.still_installed.is_empty());
        assert!(!fx.env.project.join(".agents/skills/tdd").exists());
        assert!(by_hand.exists(), "hand-installed skill survives");
        assert!(
            fx.ledger
                .get(&Ledger::key(&fx.env, Scope::Project))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn items_that_were_already_installed_are_not_owned() {
        let fx = fx();
        let dir = fx.env.project.join(".agents/skills/tdd");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: tdd\ndescription: d\n---\n",
        )
        .unwrap();
        let applied = apply_to(&fx, &NpxFake::new(&fx.env.project)).unwrap();
        assert!(applied.added.is_empty());
        unapply_from(&fx, &NpxFake::new(&fx.env.project));
        assert!(dir.exists());
    }

    #[test]
    fn a_scope_holds_one_profile_at_a_time() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        apply_to(&fx, &runner).unwrap();
        let before = *runner.calls.borrow();
        assert!(apply_to(&fx, &runner).is_err());
        assert_eq!(*runner.calls.borrow(), before, "second apply ran nothing");
    }

    #[test]
    fn a_failed_install_leaves_no_record_when_nothing_landed() {
        let fx = fx();
        let mut runner = NpxFake::new(&fx.env.project);
        runner.fail = true;
        assert!(apply_to(&fx, &runner).is_err());
        assert!(
            fx.ledger
                .get(&Ledger::key(&fx.env, Scope::Project))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn unapply_without_a_profile_does_nothing() {
        let fx = fx();
        assert!(unapply_from(&fx, &NpxFake::new(&fx.env.project)).is_none());
    }

    #[test]
    fn a_partial_install_is_recorded_so_it_can_be_unapplied() {
        let fx = fx();
        let mut p = profile();
        p.entries.push(Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: "pp".into(),
            agents: vec![crate::model::Agent::Claude],
        });
        // The skill lands, then the plugin command fails.
        struct FailsOnClaude(NpxFake);
        impl CommandRunner for FailsOnClaude {
            fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
                if program == "claude" {
                    return Ok(Output {
                        success: false,
                        stderr: "no".into(),
                    });
                }
                self.0.run(program, args, cwd)
            }
        }
        let runner = FailsOnClaude(NpxFake::new(&fx.env.project));
        let res = apply(
            &registry(),
            &p,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            &runner,
            &mut |_| {},
        );
        assert!(res.is_err());
        let record = fx
            .ledger
            .get(&Ledger::key(&fx.env, Scope::Project))
            .unwrap()
            .unwrap();
        assert_eq!(record.items.len(), 1);
    }

    #[test]
    fn corrupt_ledger_is_an_error_not_an_empty_one() {
        let fx = fx();
        fs::create_dir_all(fx.env.skillz_dir()).unwrap();
        fs::write(fx.env.skillz_dir().join("applied.json"), "{nope").unwrap();
        assert!(fx.ledger.get("x").is_err());
        assert!(apply_to(&fx, &NpxFake::new(&fx.env.project)).is_err());
    }

    #[test]
    fn ledger_keeps_scopes_apart() {
        let fx = fx();
        let rec = |p: &str| Record {
            profile: p.into(),
            items: vec![],
        };
        fx.ledger.put("global", Some(rec("a"))).unwrap();
        fx.ledger.put("/proj", Some(rec("b"))).unwrap();
        fx.ledger.put("global", None).unwrap();
        assert!(fx.ledger.get("global").unwrap().is_none());
        assert_eq!(fx.ledger.get("/proj").unwrap(), Some(rec("b")));
    }
}
