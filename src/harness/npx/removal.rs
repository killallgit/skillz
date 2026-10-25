use std::path::{Path, PathBuf};

use super::SkillDir;
use super::entry::{Entry, canonical_or};
use crate::env::Env;
use crate::harness::action::{Action, Edit, RemovalTarget};
use crate::harness::npx::commands;
use crate::model::Scope;

/// Links first, then the canonical dir if we own it, then the lock entry. No duplicates.
/// Every `RemovePath` is under an `owned` root.
pub(super) fn removal(
    entries: &[Entry],
    canonical: &Path,
    owned: &[PathBuf],
    lock: Option<(PathBuf, String)>,
) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::new();
    for t in removal_targets(entries, canonical, owned) {
        let a = Action::RemovePath(t);
        if !actions.contains(&a) {
            actions.push(a);
        }
    }
    actions.extend(lock.map(|(file, name)| Action::Edit {
        file,
        edit: Edit::JsonRemoveKey {
            path: commands::lock_key(&name),
        },
    }));
    actions
}

fn removal_targets(entries: &[Entry], canonical: &Path, owned: &[PathBuf]) -> Vec<RemovalTarget> {
    let store = owned
        .iter()
        .find_map(|r| RemovalTarget::new(r, canonical.strip_prefix(r).ok()?));
    entries
        .iter()
        .filter(|e| e.link != canonical)
        .filter_map(|e| {
            let dir = e.link.parent().filter(|d| owned.iter().any(|r| r == d))?;
            RemovalTarget::new(dir, [&e.name])
        })
        .chain(store)
        .collect()
}

/// Dirs whose contents skillz may delete for `scope`: that scope's skill dirs and its
/// `.agents/skills` store. Anything outside (e.g. a user's checkout) is never removed, nor
/// is a project dir that resolves outside the project (e.g. symlinked into `~`).
pub(super) fn owned_roots(env: &Env, dirs: &[SkillDir], scope: Scope) -> Vec<PathBuf> {
    let project = canonical_or(&env.project);
    dirs.iter()
        .filter(|d| d.scope == scope)
        .map(|d| d.path.clone())
        .chain([commands::store_dir(env, scope)])
        .map(|p| canonical_or(&p))
        .filter(|r| scope == Scope::Global || r.starts_with(&project))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::discover;
    use super::super::testutil::*;
    use crate::model::{Agent, Scope};

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
}
