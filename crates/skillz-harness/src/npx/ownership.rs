use std::path::Path;

use super::discover::Ctx;
use super::entry::canonical_or;
use crate::npx::commands::{self, LockEntry};

/// A skill the npx lock claims: its name in the lock and what the lock records of it.
pub(super) struct Locked {
    pub name: String,
    pub entry: LockEntry,
}

/// npx keys its lock by the dir name in its store. With `--copy` it writes real dirs into
/// agent skill dirs instead; those own the entry only when the store has no such skill.
pub(super) fn lock_ownership(cx: &Ctx, canonical: &Path) -> Option<Locked> {
    let store = canonical_or(&commands::store_dir(cx.env, cx.scope));
    let parent = canonical.parent();
    canonical
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| parent == Some(store.as_path()) || is_copied(cx, &store, parent, n))
        .and_then(|n| {
            Some(Locked {
                name: n.to_owned(),
                entry: cx.lock.get(n)?.clone(),
            })
        })
}

fn is_copied(cx: &Ctx, store: &Path, parent: Option<&Path>, name: &str) -> bool {
    !store.join(name).exists()
        && cx
            .dirs
            .iter()
            .any(|d| d.scope == cx.scope && parent == Some(canonical_or(&d.path).as_path()))
}

#[cfg(test)]
mod tests {
    use super::super::discover;
    use super::super::testutil::*;
    use crate::action::Action;
    use crate::env::Env;
    use crate::model::{Agent, Installer, Scope};
    use std::path::Path;

    #[test]
    fn a_locked_skill_carries_its_link_and_content_hash() {
        let f = fixture();
        let canon = f.env.home.join(".agents/skills/tdd");
        skill(&canon);
        write_lock(
            &f.env.home.join(".agents/.skill-lock.json"),
            r#"{"skills":{"tdd":{"source":"o/r","sourceUrl":"https://h/o/r.git","skillFolderHash":"abc"}}}"#,
        );

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs[0].origin.link.as_deref(), Some("https://h/o/r.git"));
        assert_eq!(augs[0].origin.version.as_deref(), Some("abc"));
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
    fn corrupt_lock_is_an_error_not_an_empty_lock() {
        let f = fixture();
        skill(&f.env.home.join(".agents/skills/x"));
        write_lock(&f.env.home.join(".agents/.skill-lock.json"), "{not json");

        assert!(discover(&f.env, &global_dirs(&f.env)).is_err());
    }

    #[test]
    fn a_corrupt_lock_in_a_scope_not_discovered_is_ignored() {
        let f = fixture();
        skill(&f.env.home.join(".agents/skills/x"));
        std::fs::create_dir_all(&f.env.project).unwrap();
        write_lock(&f.env.project.join("skills-lock.json"), "{not json");

        let augs = discover(&f.env, &global_dirs(&f.env)).unwrap();

        assert_eq!(augs.len(), 1);
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
            skill(&h.join(".agents/skills").join(name));
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
