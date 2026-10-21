//! Load a profile into a scope, swap it for another, or take it out. What each entry added is
//! found by reading the inventory before and after it, and recorded in a ledger. A switch
//! compares the ledger's entries with the target's: dropped entries lose exactly the items they
//! added, kept entries are left alone, new ones are installed. Nothing that was already there or
//! was installed by hand since is touched.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::action::{self, Action, Report};
use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::model::{Augmentation, Kind, Scope};
use crate::profile::{Entry, Profile};
use crate::provision::{self, Unsupported};
use crate::remove::{self, Outcome, Plan};
use crate::runner::CommandRunner;

/// One item a profile added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    pub name: String,
    pub kind: Kind,
}

impl Owned {
    fn of(item: &Augmentation) -> Self {
        Owned {
            id: item.id.clone(),
            name: item.name.clone(),
            kind: item.kind,
        }
    }
}

/// An entry (with its agents resolved) and the items its install added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loaded {
    pub entry: Entry,
    pub items: Vec<Owned>,
}

/// What a profile put into one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub profile: String,
    pub entries: Vec<Loaded>,
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

/// What moving a scope to a target profile involves. Build it with `plan`, show it, then
/// `run` it.
#[derive(Debug)]
pub struct Switch {
    pub from: Option<String>,
    /// `None` to unapply.
    pub to: Option<String>,
    /// Entries in both: left as they are.
    pub keep: Vec<Loaded>,
    /// Entries only in the old profile.
    pub drop: Vec<Loaded>,
    /// The installed items the dropped entries added, to remove first.
    pub removals: Vec<Augmentation>,
    /// Entries only in the new profile, each with its install steps.
    pub installs: Vec<(Entry, Vec<Action>)>,
    pub unsupported: Vec<Unsupported>,
}

impl Switch {
    pub fn is_empty(&self) -> bool {
        self.drop.is_empty() && self.installs.is_empty() && self.unsupported.is_empty()
    }

    /// The removal and install steps in the order `run` takes them.
    pub fn steps(&self) -> Vec<Action> {
        let refs: Vec<&Augmentation> = self.removals.iter().collect();
        Plan::new(&refs, &[])
            .steps
            .into_iter()
            .chain(self.installs.iter().flat_map(|(_, steps)| steps.clone()))
            .collect()
    }
}

/// What a scope needs to become `target` (`None` = nothing loaded). Pure apart from reading
/// the ledger and the inventory.
pub fn plan(
    adapters: &[Box<dyn AgentAdapter>],
    target: Option<&Profile>,
    scope: Scope,
    env: &Env,
    ledger: &Ledger,
) -> anyhow::Result<Switch> {
    let record = ledger.get(&Ledger::key(env, scope))?;
    let wanted = target.map(Profile::resolved).unwrap_or_default();
    let current = record
        .as_ref()
        .map(|r| r.entries.as_slice())
        .unwrap_or_default();
    let (mut keep, drop): (Vec<Loaded>, Vec<Loaded>) = current
        .iter()
        .cloned()
        .partition(|l| wanted.contains(&l.entry));
    let add: Vec<&Entry> = wanted
        .iter()
        .enumerate()
        .filter(|(i, e)| !current.iter().any(|l| &l.entry == *e) && !wanted[..*i].contains(e))
        .map(|(_, e)| e)
        .collect();
    let mut drop = drop;
    keep_shared_marketplaces(&mut keep, &mut drop);

    let installed = agents::inventory(adapters, env, &[scope]).items;
    let dropped: HashSet<&str> = drop
        .iter()
        .flat_map(|l| l.items.iter().map(|o| o.id.as_str()))
        .collect();
    let removals = installed
        .iter()
        .filter(|i| dropped.contains(i.id.as_str()))
        .cloned()
        .collect();

    let mut installs = Vec::new();
    let mut unsupported = Vec::new();
    for entry in add {
        let mut single = Profile::new("");
        single.name = "entry".into();
        single.entries = vec![entry.clone()];
        let planned = provision::plan(adapters, &single, scope, env)?;
        unsupported.extend(planned.unsupported);
        installs.push((entry.clone(), planned.steps));
    }
    Ok(Switch {
        from: record.map(|r| r.profile),
        to: target.map(|p| p.name.clone()),
        keep,
        drop,
        removals,
        installs,
        unsupported,
    })
}

/// A marketplace is added once, by whichever entry came first. When that entry goes but another
/// kept entry still has a plugin from it, the marketplace stays and the kept entry owns it.
fn keep_shared_marketplaces(keep: &mut [Loaded], drop: &mut [Loaded]) {
    for dropped in drop.iter_mut() {
        let (shared, rest): (Vec<Owned>, Vec<Owned>) =
            std::mem::take(&mut dropped.items).into_iter().partition(|o| {
                o.kind == Kind::Marketplace
                    && keep.iter().any(|k| {
                        matches!(&k.entry, Entry::Plugin { marketplace, .. } if *marketplace == o.name)
                    })
            });
        dropped.items = rest;
        for item in shared {
            let owner = keep.iter_mut().find(|k| {
                matches!(&k.entry, Entry::Plugin { marketplace, .. } if *marketplace == item.name)
            });
            if let Some(owner) = owner {
                owner.items.push(item);
            }
        }
    }
}

/// What a switch did.
#[derive(Debug)]
pub struct Done {
    /// `None` when nothing needed removing.
    pub removed: Option<Outcome>,
    pub installed: Report,
    /// Items the new entries added.
    pub added: Vec<Owned>,
}

/// Remove the dropped entries' items, install the new entries one at a time, and record what
/// each added. A step that fails still records what landed before it, then returns the error.
pub fn run(
    adapters: &[Box<dyn AgentAdapter>],
    switch: &Switch,
    scope: Scope,
    env: &Env,
    ledger: &Ledger,
    runner: &dyn CommandRunner,
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Done> {
    let key = Ledger::key(env, scope);
    let mut removed = None;
    let mut leftover: Vec<Loaded> = vec![];
    if !switch.removals.is_empty() {
        let refs: Vec<&Augmentation> = switch.removals.iter().collect();
        let outcome = remove::run(
            &Plan::new(&refs, &[]),
            adapters,
            env,
            &[scope],
            runner,
            on_step,
        )?;
        leftover = switch
            .drop
            .iter()
            .filter_map(|l| {
                let items: Vec<Owned> = l
                    .items
                    .iter()
                    .filter(|o| outcome.still_installed.contains(&o.name))
                    .cloned()
                    .collect();
                (!items.is_empty()).then(|| Loaded {
                    entry: l.entry.clone(),
                    items,
                })
            })
            .collect();
        removed = Some(outcome);
    }

    let mut entries = switch.keep.clone();
    entries.extend(leftover);
    let mut installed = Report::default();
    let mut added = vec![];
    let mut failure = None;
    for (entry, steps) in &switch.installs {
        let inventory = || agents::inventory(adapters, env, &[scope]).items;
        let before: HashSet<String> = inventory().into_iter().map(|i| i.id).collect();
        let ran = action::execute_with(steps, runner, on_step);
        let items: Vec<Owned> = inventory()
            .iter()
            .filter(|i| !before.contains(&i.id))
            .map(Owned::of)
            .collect();
        added.extend(items.iter().cloned());
        if ran.is_ok() || !items.is_empty() {
            entries.push(Loaded {
                entry: entry.clone(),
                items,
            });
        }
        match ran {
            Ok(report) => {
                installed.ran.extend(report.ran);
                installed.skipped.extend(report.skipped);
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }

    let profile = switch.to.clone().or_else(|| switch.from.clone());
    let nothing = entries.is_empty() && (switch.to.is_none() || failure.is_some());
    let record = match (profile, nothing) {
        (Some(profile), false) => Some(Record { profile, entries }),
        _ => None,
    };
    ledger.put(&key, record)?;
    match failure {
        Some(e) => Err(e),
        None => Ok(Done {
            removed,
            installed,
            added,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;
    use crate::model::Agent;
    use crate::runner::Output;
    use std::cell::RefCell;
    use std::path::Path;

    /// Installs the skills named by `--skill` into the project's npx store when asked to
    /// `skills add`, like the real tool would; every command fails when `fail` is set.
    struct NpxFake {
        project: PathBuf,
        calls: RefCell<Vec<Vec<String>>>,
        fail: bool,
    }

    impl NpxFake {
        fn new(project: &Path) -> Self {
            NpxFake {
                project: project.into(),
                calls: RefCell::default(),
                fail: false,
            }
        }

        fn count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    fn write_skill(root: &Path, name: &str) {
        let dir = root.join(".agents/skills").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: d\n---\n"),
        )
        .unwrap();
    }

    impl CommandRunner for NpxFake {
        fn run(&self, _program: &str, args: &[String], _cwd: &Path) -> anyhow::Result<Output> {
            self.calls.borrow_mut().push(args.to_vec());
            if !self.fail && args.iter().any(|a| a == "add") {
                for pair in args.windows(2).filter(|w| w[0] == "--skill") {
                    write_skill(&self.project, &pair[1]);
                }
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

    /// A profile with one skill entry per name.
    fn profile(name: &str, skills: &[&str]) -> Profile {
        let mut p = Profile::new(name);
        p.entries = skills
            .iter()
            .map(|s| Entry::Skill {
                source: "o/r".into(),
                names: vec![s.to_string()],
                agents: vec![],
            })
            .collect();
        p
    }

    fn switch_to(fx: &Fx, target: Option<&Profile>, runner: &dyn CommandRunner) -> Done {
        let sw = plan(&registry(), target, Scope::Project, &fx.env, &fx.ledger).unwrap();
        run(
            &registry(),
            &sw,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            runner,
            &mut |_| {},
        )
        .unwrap()
    }

    fn record(fx: &Fx) -> Option<Record> {
        fx.ledger
            .get(&Ledger::key(&fx.env, Scope::Project))
            .unwrap()
    }

    fn owned_names(fx: &Fx) -> Vec<String> {
        let mut names: Vec<String> = record(fx)
            .into_iter()
            .flat_map(|r| r.entries)
            .flat_map(|l| l.items)
            .map(|o| o.name)
            .collect();
        names.sort();
        names
    }

    fn installed(fx: &Fx, name: &str) -> bool {
        fx.env.project.join(".agents/skills").join(name).exists()
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

        let done = switch_to(&fx, Some(&profile("web", &["tdd"])), &runner);
        assert_eq!(done.added.len(), 1);
        assert_eq!(record(&fx).unwrap().profile, "web");

        let done = switch_to(&fx, None, &runner);
        assert!(done.removed.unwrap().still_installed.is_empty());
        assert!(!installed(&fx, "tdd"));
        assert!(by_hand.exists(), "hand-installed skill survives");
        assert!(record(&fx).is_none());
    }

    #[test]
    fn items_that_were_already_installed_are_not_owned() {
        let fx = fx();
        write_skill(&fx.env.project, "tdd");
        let runner = NpxFake::new(&fx.env.project);
        let done = switch_to(&fx, Some(&profile("web", &["tdd"])), &runner);
        assert!(done.added.is_empty());
        switch_to(&fx, None, &runner);
        assert!(installed(&fx, "tdd"));
    }

    #[test]
    fn applying_the_same_profile_again_changes_nothing() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        let web = profile("web", &["tdd"]);
        switch_to(&fx, Some(&web), &runner);
        let sw = plan(&registry(), Some(&web), Scope::Project, &fx.env, &fx.ledger).unwrap();
        assert!(sw.is_empty());
        assert!(sw.steps().is_empty());
        assert_eq!(runner.count(), 1);
    }

    #[test]
    fn swapping_touches_only_the_difference() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        switch_to(&fx, Some(&profile("a", &["tdd", "old"])), &runner);
        assert_eq!(runner.count(), 2);

        let b = profile("b", &["tdd", "new"]);
        let sw = plan(&registry(), Some(&b), Scope::Project, &fx.env, &fx.ledger).unwrap();
        assert_eq!((sw.keep.len(), sw.drop.len(), sw.installs.len()), (1, 1, 1));
        assert_eq!(
            (sw.from.as_deref(), sw.to.as_deref()),
            (Some("a"), Some("b"))
        );
        run(
            &registry(),
            &sw,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            &runner,
            &mut |_| {},
        )
        .unwrap();

        assert_eq!(runner.count(), 3, "only the new skill was installed");
        assert!(installed(&fx, "tdd") && installed(&fx, "new") && !installed(&fx, "old"));
        assert_eq!(owned_names(&fx), ["new", "tdd"]);
        assert_eq!(record(&fx).unwrap().profile, "b");
    }

    #[test]
    fn duplicate_entries_install_once() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        switch_to(&fx, Some(&profile("a", &["tdd", "tdd"])), &runner);
        assert_eq!(runner.count(), 1);
    }

    #[test]
    fn a_profile_whose_agents_differ_is_a_different_entry() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        switch_to(&fx, Some(&profile("a", &["tdd"])), &runner);
        let mut b = profile("b", &["tdd"]);
        b.agents = vec![Agent::Codex];
        let sw = plan(&registry(), Some(&b), Scope::Project, &fx.env, &fx.ledger).unwrap();
        assert_eq!((sw.keep.len(), sw.drop.len(), sw.installs.len()), (0, 1, 1));
    }

    #[test]
    fn a_failed_install_leaves_no_record_when_nothing_landed() {
        let fx = fx();
        let mut runner = NpxFake::new(&fx.env.project);
        runner.fail = true;
        let sw = plan(
            &registry(),
            Some(&profile("a", &["tdd"])),
            Scope::Project,
            &fx.env,
            &fx.ledger,
        )
        .unwrap();
        let res = run(
            &registry(),
            &sw,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            &runner,
            &mut |_| {},
        );
        assert!(res.is_err());
        assert!(record(&fx).is_none());
    }

    #[test]
    fn unapply_without_a_profile_does_nothing() {
        let fx = fx();
        let runner = NpxFake::new(&fx.env.project);
        let sw = plan(&registry(), None, Scope::Project, &fx.env, &fx.ledger).unwrap();
        assert!(sw.is_empty());
        run(
            &registry(),
            &sw,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            &runner,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(runner.count(), 0);
        assert!(record(&fx).is_none());
    }

    #[test]
    fn a_partial_install_is_recorded_so_it_can_be_unapplied() {
        let fx = fx();
        let mut p = profile("web", &["tdd"]);
        p.entries.push(Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: "pp".into(),
            agents: vec![Agent::Claude],
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
        let sw = plan(&registry(), Some(&p), Scope::Project, &fx.env, &fx.ledger).unwrap();
        let res = run(
            &registry(),
            &sw,
            Scope::Project,
            &fx.env,
            &fx.ledger,
            &runner,
            &mut |_| {},
        );
        assert!(res.is_err());
        assert_eq!(owned_names(&fx), ["tdd"]);
        switch_to(&fx, None, &runner);
        assert!(!installed(&fx, "tdd"));
    }

    #[test]
    fn corrupt_ledger_is_an_error_not_an_empty_one() {
        let fx = fx();
        fs::create_dir_all(fx.env.skillz_dir()).unwrap();
        fs::write(fx.env.skillz_dir().join("applied.json"), "{nope").unwrap();
        assert!(fx.ledger.get("x").is_err());
        assert!(plan(&registry(), None, Scope::Project, &fx.env, &fx.ledger).is_err());
    }

    #[test]
    fn ledger_keeps_scopes_apart() {
        let fx = fx();
        let rec = |p: &str| Record {
            profile: p.into(),
            entries: vec![],
        };
        fx.ledger.put("global", Some(rec("a"))).unwrap();
        fx.ledger.put("/proj", Some(rec("b"))).unwrap();
        fx.ledger.put("global", None).unwrap();
        assert!(fx.ledger.get("global").unwrap().is_none());
        assert_eq!(fx.ledger.get("/proj").unwrap(), Some(rec("b")));
    }

    #[test]
    fn a_marketplace_stays_with_the_kept_entry_that_still_uses_it() {
        let plugin = |name: &str| Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: name.into(),
            agents: vec![Agent::Claude],
        };
        let owned = |name: &str, kind| Owned {
            id: format!("{kind:?}:{name}"),
            name: name.into(),
            kind,
        };
        let mut keep = vec![Loaded {
            entry: plugin("b"),
            items: vec![owned("b", Kind::Plugin)],
        }];
        let mut drop = vec![Loaded {
            entry: plugin("a"),
            items: vec![owned("mp", Kind::Marketplace), owned("a", Kind::Plugin)],
        }];
        keep_shared_marketplaces(&mut keep, &mut drop);
        assert_eq!(drop[0].items.len(), 1);
        assert_eq!(drop[0].items[0].name, "a");
        assert_eq!(keep[0].items.len(), 2);

        let mut none: Vec<Loaded> = vec![];
        keep_shared_marketplaces(&mut none, &mut drop);
        assert_eq!(
            drop[0].items.len(),
            1,
            "a lone plugin's items are all dropped"
        );
    }
}
