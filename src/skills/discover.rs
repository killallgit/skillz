use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::SkillDir;
use super::entry::{Entry, entries};
use super::ownership::{Locked, lock_ownership};
use super::removal::{owned_roots, removal};
use crate::action::Action;
use crate::env::Env;
use crate::model::{Agent, Augmentation, Installer, Kind, Origin, Scope};
use crate::npx_skills::{self, LockEntry};

type Groups = BTreeMap<(Scope, PathBuf), Vec<Entry>>;

/// What building one scope's skills needs to know.
pub(super) struct Ctx<'a> {
    pub env: &'a Env,
    pub dirs: &'a [SkillDir],
    pub scope: Scope,
    pub lock: &'a BTreeMap<String, LockEntry>,
}

/// Installed skills across all `dirs`, deduplicated by canonical path.
pub fn discover(env: &Env, dirs: &[SkillDir]) -> anyhow::Result<Vec<Augmentation>> {
    let groups = group_by_canonical(dirs);
    let global: BTreeSet<&PathBuf> = global_targets(&groups);
    let locks = read_locks(env);

    let mut out: Vec<Augmentation> = groups
        .iter()
        .map(|((scope, canonical), es)| {
            let cx = Ctx {
                env,
                dirs,
                scope: *scope,
                lock: &locks[scope],
            };
            // Only a project skill can collide with a global one (same canonical target).
            let suffix = *scope == Scope::Project && global.contains(canonical);
            augmentation(&cx, canonical, es, suffix)
        })
        .collect();
    out.sort_by(|a, b| (&a.group, &a.name, &a.id).cmp(&(&b.group, &b.name, &b.id)));
    Ok(out)
}

fn group_by_canonical(dirs: &[SkillDir]) -> Groups {
    let mut groups = Groups::new();
    for dir in dirs {
        for e in entries(dir) {
            groups
                .entry((dir.scope, e.canonical.clone()))
                .or_default()
                .push(e);
        }
    }
    groups
}

fn global_targets(groups: &Groups) -> BTreeSet<&PathBuf> {
    groups
        .keys()
        .filter(|(s, _)| *s == Scope::Global)
        .map(|(_, c)| c)
        .collect()
}

fn read_locks(env: &Env) -> BTreeMap<Scope, BTreeMap<String, LockEntry>> {
    [Scope::Global, Scope::Project]
        .into_iter()
        .map(|s| (s, npx_skills::read_lock(env, s)))
        .collect()
}

fn augmentation(cx: &Ctx, canonical: &Path, entries: &[Entry], suffix: bool) -> Augmentation {
    let locked = lock_ownership(cx, canonical);
    let agents = agents_of(entries);
    let source = locked.as_ref().map(|l| l.entry.source.clone());
    let mut plan = removal_plan(cx, canonical, entries, locked.as_ref());
    plan.splice(0..0, native_uninstall(cx, locked.as_ref(), &agents));
    Augmentation {
        hooks: Vec::new(),
        id: skill_id(canonical, cx.scope, suffix),
        name: entries[0].name.clone(),
        kind: Kind::Skill,
        agents,
        scope: cx.scope,
        group: source.clone().unwrap_or_else(|| "local".into()),
        source,
        installer: installer_of(locked.as_ref()),
        origin: origin_of(locked.as_ref()),
        children: vec![],
        removal: plan,
    }
}

fn skill_id(canonical: &Path, scope: Scope, suffix: bool) -> String {
    match suffix {
        true => format!("skill:{}:{scope}", canonical.display()),
        false => format!("skill:{}", canonical.display()),
    }
}

fn agents_of(entries: &[Entry]) -> Vec<Agent> {
    entries
        .iter()
        .map(|e| e.agent)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn origin_of(locked: Option<&Locked>) -> Origin {
    Origin {
        link: locked.and_then(|l| l.entry.link.clone()),
        version: locked.and_then(|l| l.entry.hash.clone()),
    }
}

fn installer_of(locked: Option<&Locked>) -> Installer {
    match locked {
        Some(_) => Installer::Npx,
        None => Installer::Local,
    }
}

/// The CLI that installed it goes first; the file steps then finish what it left.
fn native_uninstall(cx: &Ctx, locked: Option<&Locked>, agents: &[Agent]) -> Option<Action> {
    let locked = locked?;
    let (program, args) = npx_skills::remove_command(&locked.name, agents, cx.scope);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    Action::native(&program, &args, &[&locked.name], cx.env.root(cx.scope))
}

fn removal_plan(
    cx: &Ctx,
    canonical: &Path,
    entries: &[Entry],
    locked: Option<&Locked>,
) -> Vec<Action> {
    let lock = locked.map(|l| (npx_skills::lock_path(cx.env, cx.scope), l.name.clone()));
    removal(
        entries,
        canonical,
        &owned_roots(cx.env, cx.dirs, cx.scope),
        lock,
    )
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;

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
}
