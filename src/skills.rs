use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::model::{Agent, Augmentation, Installer, Kind, Scope};
use crate::npx_skills;

/// A directory an agent loads skills from (each skill is a subdir with `SKILL.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub agent: Agent,
    pub scope: Scope,
    pub path: PathBuf,
}

/// One skill entry as seen from one agent dir.
#[derive(Debug, Clone)]
struct Entry {
    agent: Agent,
    name: String,
    /// The entry inside the agent dir (a symlink or a real dir).
    link: PathBuf,
    /// Resolved skill dir; the link itself when the link is dangling.
    canonical: PathBuf,
}

/// Installed skills across all `dirs`, deduplicated by canonical path.
pub fn discover(env: &Env, dirs: &[SkillDir]) -> anyhow::Result<Vec<Augmentation>> {
    let mut groups: BTreeMap<(Scope, PathBuf), Vec<Entry>> = BTreeMap::new();
    for dir in dirs {
        for e in entries(dir) {
            groups
                .entry((dir.scope, e.canonical.clone()))
                .or_default()
                .push(e);
        }
    }
    let global: BTreeSet<&PathBuf> = groups
        .keys()
        .filter(|(s, _)| *s == Scope::Global)
        .map(|(_, c)| c)
        .collect();
    let locks: BTreeMap<Scope, BTreeMap<String, String>> = [Scope::Global, Scope::Project]
        .into_iter()
        .map(|s| (s, npx_skills::read_lock(env, s)))
        .collect();

    let mut out: Vec<Augmentation> = groups
        .iter()
        .map(|((scope, canonical), es)| {
            // Only a project skill can collide with a global one (same canonical target).
            let suffix = *scope == Scope::Project && global.contains(canonical);
            augmentation(env, dirs, *scope, canonical, es, &locks[scope], suffix)
        })
        .collect();
    out.sort_by(|a, b| (&a.group, &a.name, &a.id).cmp(&(&b.group, &b.name, &b.id)));
    Ok(out)
}

fn augmentation(
    env: &Env,
    dirs: &[SkillDir],
    scope: Scope,
    canonical: &Path,
    entries: &[Entry],
    lock: &BTreeMap<String, String>,
    suffix: bool,
) -> Augmentation {
    let name = entries[0].name.clone();
    // npx keys its lock by the dir name in its store. With `--copy` it writes real dirs into
    // agent skill dirs instead; those own the entry only when the store has no such skill.
    let store = canonical_or(&npx_skills::store_dir(env, scope));
    let parent = canonical.parent();
    let copied = |n: &str| {
        !store.join(n).exists()
            && dirs
                .iter()
                .any(|d| d.scope == scope && parent == Some(canonical_or(&d.path).as_path()))
    };
    let locked = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| parent == Some(store.as_path()) || copied(n))
        .and_then(|n| Some((n.to_owned(), lock.get(n)?.clone())));
    let source = locked.as_ref().map(|(_, s)| s.clone());
    let agents: Vec<Agent> = entries
        .iter()
        .map(|e| e.agent)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let id = match suffix {
        true => format!("skill:{}:{scope}", canonical.display()),
        false => format!("skill:{}", canonical.display()),
    };
    let installer = if locked.is_some() {
        Installer::Npx
    } else {
        Installer::Local
    };
    // The CLI that installed it goes first; the file steps then finish what it left.
    let native = locked.as_ref().and_then(|(name, _)| {
        let (program, args) = npx_skills::remove_command(name, &agents, scope);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        Action::native(&program, &args, &[name], env.root(scope))
    });
    let mut removal = removal(
        entries,
        canonical,
        &owned_roots(env, dirs, scope),
        locked.map(|(n, _)| (npx_skills::lock_path(env, scope), n)),
    );
    removal.splice(0..0, native);
    Augmentation {
        id,
        name,
        kind: Kind::Skill,
        agents,
        scope,
        group: source.clone().unwrap_or_else(|| "local".into()),
        source,
        installer,
        children: vec![],
        removal,
    }
}

/// Links first, then the canonical dir if we own it, then the lock entry. No duplicates.
/// Every `RemovePath` is under an `owned` root.
fn removal(
    entries: &[Entry],
    canonical: &Path,
    owned: &[PathBuf],
    lock: Option<(PathBuf, String)>,
) -> Vec<Action> {
    let store = owned
        .iter()
        .find_map(|r| RemovalTarget::new(r, canonical.strip_prefix(r).ok()?));
    let targets = entries
        .iter()
        .filter(|e| e.link != canonical)
        .filter_map(|e| {
            let dir = e.link.parent().filter(|d| owned.iter().any(|r| r == d))?;
            RemovalTarget::new(dir, [&e.name])
        })
        .chain(store);
    let mut actions: Vec<Action> = Vec::new();
    for t in targets {
        let a = Action::RemovePath(t);
        if !actions.contains(&a) {
            actions.push(a);
        }
    }
    actions.extend(lock.map(|(file, name)| Action::Edit {
        file,
        edit: Edit::JsonRemoveKey {
            path: npx_skills::lock_key(&name),
        },
    }));
    actions
}

/// Dirs whose contents skillz may delete for `scope`: that scope's skill dirs and its
/// `.agents/skills` store. Anything outside (e.g. a user's checkout) is never removed, nor
/// is a project dir that resolves outside the project (e.g. symlinked into `~`).
fn owned_roots(env: &Env, dirs: &[SkillDir], scope: Scope) -> Vec<PathBuf> {
    let project = canonical_or(&env.project);
    dirs.iter()
        .filter(|d| d.scope == scope)
        .map(|d| d.path.clone())
        .chain([npx_skills::store_dir(env, scope)])
        .map(|p| canonical_or(&p))
        .filter(|r| scope == Scope::Global || r.starts_with(&project))
        .collect()
}

fn entries(dir: &SkillDir) -> Vec<Entry> {
    // Canonical parent makes a real (non-symlinked) skill's link equal its canonical path.
    let base = canonical_or(&dir.path);
    let Ok(read) = fs::read_dir(&base) else {
        return vec![];
    };
    let mut out: Vec<Entry> = read
        .filter_map(Result::ok)
        .filter_map(|e| entry(dir.agent, base.join(e.file_name())))
        .collect();
    out.sort_by(|a, b| a.link.cmp(&b.link));
    out
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

fn canonical_or(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A removal plan without its native-tool commands.
    fn files(plan: &[Action]) -> Vec<Action> {
        plan.iter()
            .filter(|a| !matches!(a, Action::Run { .. }))
            .cloned()
            .collect()
    }

    use crate::model::Installer;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        env: Env,
    }

    fn fixture() -> Fixture {
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let env = Env {
            home: root.join("home"),
            project: root.join("proj"),
            claude_config: None,
            codex_home: None,
        };
        fs::create_dir_all(&env.home).unwrap();
        fs::create_dir_all(&env.project).unwrap();
        Fixture { _tmp: tmp, env }
    }

    fn skill(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
    }

    fn link(target: &Path, at: &Path) {
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        symlink(target, at).unwrap();
    }

    fn sd(agent: Agent, scope: Scope, path: PathBuf) -> SkillDir {
        SkillDir { agent, scope, path }
    }

    fn global_dirs(env: &Env) -> Vec<SkillDir> {
        let h = &env.home;
        vec![
            sd(Agent::Claude, Scope::Global, h.join(".claude/skills")),
            sd(Agent::Codex, Scope::Global, h.join(".agents/skills")),
            sd(Agent::Codex, Scope::Global, h.join(".codex/skills")),
            sd(Agent::Cursor, Scope::Global, h.join(".cursor/skills")),
        ]
    }

    fn rm(p: PathBuf) -> Action {
        let target = RemovalTarget::new(p.parent().unwrap(), [p.file_name().unwrap()]);
        Action::RemovePath(target.unwrap())
    }

    fn lock_edit(file: PathBuf, name: &str) -> Action {
        Action::Edit {
            file,
            edit: Edit::JsonRemoveKey {
                path: vec!["skills".into(), name.into()],
            },
        }
    }

    fn write_lock(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn by_name<'a>(augs: &'a [Augmentation], name: &str) -> &'a Augmentation {
        augs.iter().find(|a| a.name == name).unwrap()
    }

    #[test]
    fn missing_dirs_yield_nothing() {
        let f = fixture();
        assert!(discover(&f.env, &global_dirs(&f.env)).unwrap().is_empty());
    }

    #[test]
    fn npx_skill_shared_by_symlinks_is_one_augmentation() {
        let f = fixture();
        let h = &f.env.home;
        let canon = h.join(".agents/skills/tdd");
        skill(&canon);
        link(&canon, &h.join(".claude/skills/tdd"));
        link(&canon, &h.join(".cursor/skills/tdd"));
        let lock = h.join(".agents/.skill-lock.json");
        write_lock(
            &lock,
            r#"{"version":3,"skills":{"tdd":{"source":"mattpocock/skills","sourceType":"github"}}}"#,
        );

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
        let a = &augs[0];
        assert_eq!(a.id, format!("skill:{}", canon.display()));
        assert_eq!(a.name, "tdd");
        assert_eq!(a.kind, Kind::Skill);
        assert_eq!(a.scope, Scope::Global);
        assert_eq!(a.agents, vec![Agent::Claude, Agent::Codex, Agent::Cursor]);
        assert_eq!(a.group, "mattpocock/skills");
        assert_eq!(a.source.as_deref(), Some("mattpocock/skills"));
        assert!(a.children.is_empty());
        assert_eq!(
            files(&a.removal),
            vec![
                rm(h.join(".claude/skills/tdd")),
                rm(h.join(".cursor/skills/tdd")),
                rm(canon.clone()),
                lock_edit(lock, "tdd"),
            ]
        );
    }

    #[test]
    fn lock_entry_belongs_to_the_store_dir_not_the_entry_name() {
        let f = fixture();
        let h = &f.env.home;
        let canon = h.join(".agents/skills/tdd");
        skill(&canon);
        let renamed = h.join(".claude/skills/my-tdd");
        link(&canon, &renamed);
        let mine = h.join(".codex/skills/tdd");
        skill(&mine);
        let lock = h.join(".agents/.skill-lock.json");
        write_lock(&lock, r#"{"skills":{"tdd":{"source":"o/r"}}}"#);

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        let id = |p: &Path| format!("skill:{}", p.display());
        let npx = augs.iter().find(|a| a.id == id(&canon)).unwrap();
        assert_eq!(npx.source.as_deref(), Some("o/r"));
        assert_eq!(npx.installer, Installer::Npx);
        assert_eq!(
            files(&npx.removal),
            vec![rm(renamed), rm(canon.clone()), lock_edit(lock, "tdd")]
        );
        let local = augs.iter().find(|a| a.id == id(&mine)).unwrap();
        assert_eq!(local.source, None);
        assert_eq!(local.installer, Installer::Local);
        assert_eq!(files(&local.removal), vec![rm(mine.clone())]);
    }

    #[test]
    fn npx_copy_install_without_store_copy_keeps_its_lock() {
        let f = fixture();
        let dir = f.env.home.join(".claude/skills/find-skills");
        skill(&dir);
        let lock = f.env.home.join(".agents/.skill-lock.json");
        write_lock(&lock, r#"{"skills":{"find-skills":{"source":"o/r"}}}"#);

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
        assert_eq!(augs[0].source.as_deref(), Some("o/r"));
        assert_eq!(
            files(&augs[0].removal),
            vec![rm(dir), lock_edit(lock, "find-skills")]
        );
    }

    #[test]
    fn copied_skill_without_lock_is_local() {
        let f = fixture();
        let dir = f.env.home.join(".claude/skills/mine");
        skill(&dir);

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
        assert_eq!(augs[0].group, "local");
        assert_eq!(augs[0].source, None);
        assert_eq!(augs[0].agents, vec![Agent::Claude]);
        assert_eq!(files(&augs[0].removal), vec![rm(dir)]);
    }

    #[test]
    fn outside_target_is_never_removed() {
        let f = fixture();
        let checkout = f.env.home.join("src/my-skill");
        skill(&checkout);
        let at = f.env.home.join(".claude/skills/my-skill");
        link(&checkout, &at);

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs[0].id, format!("skill:{}", checkout.display()));
        assert_eq!(files(&augs[0].removal), vec![rm(at)]);
    }

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

    #[test]
    fn corrupt_lock_is_treated_as_absent() {
        let f = fixture();
        skill(&f.env.home.join(".agents/skills/x"));
        write_lock(&f.env.home.join(".agents/.skill-lock.json"), "{not json");

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs[0].group, "local");
        assert_eq!(
            files(&augs[0].removal),
            vec![rm(f.env.home.join(".agents/skills/x"))]
        );
    }

    #[test]
    fn lock_entry_without_source_string_is_local() {
        let f = fixture();
        skill(&f.env.home.join(".agents/skills/x"));
        write_lock(
            &f.env.home.join(".agents/.skill-lock.json"),
            r#"{"version":3,"skills":{"x":{"source":7}}}"#,
        );

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs[0].source, None);
        assert_eq!(augs[0].removal.len(), 1);
    }

    #[test]
    fn project_scope_uses_project_lock_and_store() {
        let f = fixture();
        let p = &f.env.project;
        let canon = p.join(".agents/skills/review");
        skill(&canon);
        link(&canon, &p.join(".claude/skills/review"));
        let lock = p.join("skills-lock.json");
        write_lock(
            &lock,
            r#"{"version":3,"skills":{"review":{"source":"org/repo"}}}"#,
        );
        let dirs = vec![sd(Agent::Claude, Scope::Project, p.join(".claude/skills"))];

        let augs = discover(&f.env, &dirs).unwrap();

        assert_eq!(augs.len(), 1);
        let a = &augs[0];
        assert_eq!(a.scope, Scope::Project);
        assert_eq!(a.agents, vec![Agent::Claude]);
        assert_eq!(a.source.as_deref(), Some("org/repo"));
        assert_eq!(
            files(&a.removal),
            vec![
                rm(p.join(".claude/skills/review")),
                rm(canon),
                lock_edit(lock, "review"),
            ]
        );
    }

    #[test]
    fn same_canonical_in_both_scopes_stays_separate_and_project_never_deletes_global_store() {
        let f = fixture();
        let canon = f.env.home.join(".agents/skills/shared");
        skill(&canon);
        let proj_link = f.env.project.join(".claude/skills/shared");
        link(&canon, &proj_link);
        write_lock(
            &f.env.home.join(".agents/.skill-lock.json"),
            r#"{"skills":{"shared":{"source":"a/b"}}}"#,
        );
        let mut dirs = global_dirs(&f.env);
        dirs.push(sd(
            Agent::Claude,
            Scope::Project,
            f.env.project.join(".claude/skills"),
        ));

        let augs = discover(&f.env, &dirs).unwrap();

        assert_eq!(augs.len(), 2);
        let global = augs.iter().find(|a| a.scope == Scope::Global).unwrap();
        let project = augs.iter().find(|a| a.scope == Scope::Project).unwrap();
        assert_eq!(global.id, format!("skill:{}", canon.display()));
        assert_eq!(project.id, format!("skill:{}:project", canon.display()));
        assert_eq!(project.group, "local");
        assert_eq!(files(&project.removal), vec![rm(proj_link)]);
    }

    #[test]
    fn project_dirs_symlinked_into_home_never_delete_global_files() {
        let f = fixture();
        let (h, p) = (&f.env.home, &f.env.project);
        let canon = h.join(".agents/skills/tdd");
        skill(&canon);
        link(&canon, &h.join(".claude/skills/tdd"));
        link(&h.join(".agents/skills"), &p.join(".agents/skills"));
        link(&h.join(".claude/skills"), &p.join(".claude/skills"));
        let dirs = vec![
            sd(Agent::Claude, Scope::Project, p.join(".claude/skills")),
            sd(Agent::Codex, Scope::Project, p.join(".agents/skills")),
        ];

        let augs = discover(&f.env, &dirs).unwrap();

        assert_eq!(augs.len(), 1);
        assert_eq!(augs[0].scope, Scope::Project);
        assert_eq!(files(&augs[0].removal), vec![]);
    }

    #[test]
    fn symlinked_agent_dir_does_not_duplicate_actions() {
        let f = fixture();
        let h = &f.env.home;
        let canon = h.join(".agents/skills/x");
        skill(&canon);
        link(&h.join(".agents/skills"), &h.join(".claude/skills"));

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
        assert_eq!(augs[0].agents, vec![Agent::Claude, Agent::Codex]);
        assert_eq!(files(&augs[0].removal), vec![rm(canon)]);
    }

    #[test]
    fn sorted_by_group_then_name() {
        let f = fixture();
        let s = f.env.home.join(".agents/skills");
        for n in ["zeta", "alpha", "mid"] {
            skill(&s.join(n));
        }
        write_lock(
            &f.env.home.join(".agents/.skill-lock.json"),
            r#"{"skills":{"zeta":{"source":"a/a"}}}"#,
        );

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        let order: Vec<(&str, &str)> = augs
            .iter()
            .map(|a| (a.group.as_str(), a.name.as_str()))
            .collect();
        assert_eq!(
            order,
            vec![("a/a", "zeta"), ("local", "alpha"), ("local", "mid")]
        );
        assert_eq!(files(&by_name(&augs, "zeta").removal).len(), 2);
    }

    #[test]
    fn npx_skill_removes_with_npx_and_local_skill_with_files() {
        let tmp = tempfile::tempdir().unwrap();
        let h = tmp.path();
        let env = Env {
            home: h.into(),
            project: h.join("p"),
            claude_config: None,
            codex_home: None,
        };
        write_lock(
            &h.join(".agents/.skill-lock.json"),
            r#"{"version":3,"skills":{"tdd":{"source":"o/r"}}}"#,
        );
        for name in ["tdd", "mine"] {
            let dir = h.join(".agents/skills").join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
        }
        let dirs = vec![sd(Agent::Codex, Scope::Global, h.join(".agents/skills"))];
        let augs = discover(&env, &dirs).unwrap();
        let first = |name: &str| by_name(&augs, name).removal.first().cloned().unwrap();
        let Action::Run {
            program,
            args,
            cwd,
            fallback,
        } = first("tdd")
        else {
            panic!("npx skill starts with its own uninstall");
        };
        assert_eq!(program, "npx");
        assert_eq!(args[..4], ["-y", "skills", "remove", "tdd"]);
        assert_eq!((cwd, fallback), (h.to_path_buf(), true));
        assert!(!matches!(first("mine"), Action::Run { .. }));
    }
}
