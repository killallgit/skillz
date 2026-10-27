use std::path::{Component, Path, PathBuf};

use crate::action::Action;
use crate::model::{Installed, Installer, Kind};
use crate::runner::CommandRunner;
use crate::update::check::UpdateCheck;

pub(super) fn steps(item: &Installed, cwd: &Path) -> Vec<Action> {
    let args = match (item.installer, item.kind) {
        (Installer::ClaudeCli, Kind::Plugin) => vec!["plugin", "update", &item.name],
        (Installer::ClaudeCli, Kind::Marketplace) => {
            vec!["plugin", "marketplace", "update", &item.name]
        }
        _ => return vec![],
    };
    vec![Action::required(
        "claude",
        args.into_iter().map(String::from).collect(),
        cwd,
    )]
}

/// A Claude marketplace is a git clone: it has an update when its remote's head is not the
/// commit that was cloned. Needs the network; any failure reads as no update.
pub struct ClaudeMarketplaces<R> {
    pub runner: R,
    /// `<claude dir>/plugins/marketplaces`.
    pub clones: PathBuf,
}

impl<R: CommandRunner + Send + Sync> UpdateCheck for ClaudeMarketplaces<R> {
    fn available(&self, item: &Installed) -> bool {
        if (item.kind, item.installer) != (Kind::Marketplace, Installer::ClaudeCli) {
            return false;
        }
        let Some(clone) = self.clone_of(&item.name) else {
            return false;
        };
        let heads = self
            .git(&clone, &["rev-parse", "HEAD"])
            .zip(self.git(&clone, &["ls-remote", "origin", "HEAD"]));
        heads.is_some_and(|(local, remote)| {
            let remote = remote.split_whitespace().next().unwrap_or_default();
            !remote.is_empty() && remote != local.trim()
        })
    }
}

impl<R: CommandRunner> ClaudeMarketplaces<R> {
    fn clone_of(&self, name: &str) -> Option<PathBuf> {
        let plain = Path::new(name).components().count() == 1
            && matches!(
                Path::new(name).components().next(),
                Some(Component::Normal(_))
            );
        let dir = self.clones.join(name);
        (plain && dir.join(".git").exists()).then_some(dir)
    }

    fn git(&self, dir: &Path, args: &[&str]) -> Option<String> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        self.runner.capture("git", &args, dir).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Scope};
    use crate::runner::CommandRunner;
    use crate::runner::Output;
    use std::path::Path;

    struct Heads {
        local: &'static str,
        remote: &'static str,
    }

    impl CommandRunner for Heads {
        fn run(&self, _: &str, _: &[String], _: &Path) -> anyhow::Result<Output> {
            unreachable!("a check only reads")
        }

        fn capture(&self, _: &str, args: &[String], _: &Path) -> anyhow::Result<String> {
            match args[0].as_str() {
                "rev-parse" => Ok(format!("{}\n", self.local)),
                _ => Ok(format!("{}\tHEAD\n", self.remote)),
            }
        }
    }

    fn marketplace(name: &str) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
            id: format!("marketplace:{name}"),
            name: name.into(),
            kind: Kind::Marketplace,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: name.into(),
            source: None,
            installer: Installer::ClaudeCli,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn checker(dir: &Path, local: &'static str, remote: &'static str) -> impl UpdateCheck + use<> {
        std::fs::create_dir_all(dir.join("mp/.git")).unwrap();
        ClaudeMarketplaces {
            runner: Heads { local, remote },
            clones: dir.to_path_buf(),
        }
    }

    #[test]
    fn a_marketplace_whose_remote_moved_has_an_update() {
        let dir = tempfile::tempdir().unwrap();

        let moved = checker(dir.path(), "aaa", "bbb").available(&marketplace("mp"));
        let same = checker(dir.path(), "aaa", "aaa").available(&marketplace("mp"));

        assert_eq!((moved, same), (true, false));
    }

    #[test]
    fn what_cannot_be_checked_has_no_update() {
        let dir = tempfile::tempdir().unwrap();
        let check = checker(dir.path(), "aaa", "bbb");
        let mut skill = marketplace("mp");
        skill.kind = Kind::Skill;

        let results = [
            check.available(&skill),
            check.available(&marketplace("missing")),
            check.available(&marketplace("../mp")),
        ];

        assert_eq!(results, [false; 3]);
    }
}
