use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

use crate::env::Env;
use crate::matcher::pattern_matches;
use crate::model::{Agent, Scope};
use crate::prompt::Prompter;
use crate::runner::CommandRunner;

/// Which skills to install from a multi-skill source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSelection {
    /// No `--skill`/`--all`: offer marketplace plugins if the source has a marketplace,
    /// else let the user pick skills.
    Interactive,
    All,
    /// Names or globs (`*commit`); matches are confirmed before installing.
    Patterns(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// `org/repo`, a git URL, or a local path.
    pub source: String,
    pub agents: Vec<Agent>,
    pub scope: Scope,
    pub selection: SkillSelection,
}

type Cmd = (String, Vec<String>);

/// A `.claude-plugin/marketplace.json` reduced to what install needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marketplace {
    name: String,
    plugins: Vec<String>,
}

pub fn install(
    req: &InstallRequest,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<()> {
    // Sources are passed as positional args to git/npx/claude; a leading `-` would be an option.
    if req.source.trim().is_empty() || req.source.starts_with('-') {
        anyhow::bail!("invalid source: {}", req.source);
    }
    let local = env.project.join(&req.source);
    if local.exists() {
        let source = local.to_string_lossy().into_owned();
        return install_from(req, &local, &source, env, runner, prompter);
    }
    let tmp = std::env::temp_dir().join(unique_name());
    let (program, args) = clone_command(&req.source, &tmp);
    let res = run_checked(runner, &program, &args, &env.project)
        .and_then(|()| install_from(req, &tmp, &req.source, env, runner, prompter));
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

/// `src` is the tree to inspect; `source` is what we hand to `npx`/`claude` (never a tmp clone).
fn install_from(
    req: &InstallRequest,
    src: &Path,
    source: &str,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<()> {
    let marketplace = read_marketplace(src)?;
    let mp = match marketplace {
        Some(mp)
            if req.selection == SkillSelection::Interactive
                && req.agents.contains(&Agent::Claude) =>
        {
            mp
        }
        _ => {
            return install_skills(
                &req.selection,
                &req.agents,
                req.scope,
                src,
                source,
                env,
                runner,
                prompter,
            );
        }
    };

    let picked = prompter.multi_select("Plugins to install", &mp.plugins)?;
    if picked.is_empty() {
        return Ok(());
    }
    let chosen: Vec<String> = picked.iter().map(|&i| mp.plugins[i].clone()).collect();
    for (program, args) in marketplace_commands(source, &mp.name, &chosen, req.scope) {
        run_checked(runner, &program, &args, &env.project)?;
    }

    let others: Vec<Agent> = req
        .agents
        .iter()
        .copied()
        .filter(|a| *a != Agent::Claude)
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    install_skills(
        &SkillSelection::All,
        &others,
        req.scope,
        src,
        source,
        env,
        runner,
        prompter,
    )
}

#[allow(clippy::too_many_arguments)]
fn install_skills(
    selection: &SkillSelection,
    agents: &[Agent],
    scope: Scope,
    src: &Path,
    source: &str,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<()> {
    let names: Vec<String> = scan_available(src)?.into_iter().map(|s| s.name).collect();
    let chosen = match selection {
        SkillSelection::Interactive => {
            let picked = prompter.multi_select("Skills to install", &names)?;
            if picked.is_empty() {
                return Ok(());
            }
            picked.iter().map(|&i| names[i].clone()).collect()
        }
        _ => {
            let chosen = select_skills(&names, selection);
            anyhow::ensure!(!chosen.is_empty(), "no skills to install from {source}");
            if matches!(selection, SkillSelection::Patterns(_))
                && !prompter.confirm(&format!("Install {}?", chosen.join(", ")))?
            {
                return Ok(());
            }
            chosen
        }
    };
    let (program, args) = skills_command(source, &chosen, agents, scope);
    run_checked(runner, &program, &args, &env.project)
}

/// Skill names picked by a non-interactive selection. `Interactive` yields every candidate.
fn select_skills(names: &[String], selection: &SkillSelection) -> Vec<String> {
    match selection {
        SkillSelection::Patterns(patterns) => {
            let pats: Vec<&str> = patterns
                .iter()
                .flat_map(|p| p.split(','))
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            names
                .iter()
                .filter(|n| pats.iter().any(|p| pattern_matches(p, n)))
                .cloned()
                .collect()
        }
        SkillSelection::All | SkillSelection::Interactive => names.to_vec(),
    }
}

fn agent_id(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude-code",
        Agent::Codex => "codex",
        Agent::Cursor => "cursor",
    }
}

fn skills_command(source: &str, skills: &[String], agents: &[Agent], scope: Scope) -> Cmd {
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

fn marketplace_commands(
    source: &str,
    marketplace: &str,
    plugins: &[String],
    scope: Scope,
) -> Vec<Cmd> {
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "project",
    };
    let claude = |args: &[&str]| {
        (
            "claude".to_string(),
            args.iter().map(|s| s.to_string()).collect(),
        )
    };
    std::iter::once(claude(&["plugin", "marketplace", "add", source]))
        .chain(plugins.iter().map(|p| {
            claude(&[
                "plugin",
                "install",
                &format!("{p}@{marketplace}"),
                "--scope",
                scope,
            ])
        }))
        .collect()
}

fn clone_url(source: &str) -> String {
    if source.contains("://") || source.starts_with("git@") {
        source.to_string()
    } else {
        format!("https://github.com/{source}.git")
    }
}

fn clone_command(source: &str, dest: &Path) -> Cmd {
    let args = [
        "clone",
        "--depth",
        "1",
        &clone_url(source),
        &dest.to_string_lossy(),
    ]
    .map(String::from)
    .to_vec();
    ("git".into(), args)
}

fn unique_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("skillz-install-{}-{nanos}", std::process::id())
}

/// Hide `user:token@` credentials in URLs before echoing a command.
fn redact(arg: &str) -> String {
    match (arg.find("://"), arg.find('@')) {
        (Some(scheme), Some(at)) if at > scheme && !arg[scheme + 3..at].contains('/') => {
            format!("{}://***{}", &arg[..scheme], &arg[at..])
        }
        _ => arg.to_string(),
    }
}

fn run_checked(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> anyhow::Result<()> {
    let shown: Vec<String> = args.iter().map(|a| redact(a)).collect();
    eprintln!("$ {program} {}", shown.join(" "));
    let out = runner.run(program, args, cwd)?;
    anyhow::ensure!(
        out.success,
        "`{program} {}` failed: {}",
        args.join(" "),
        out.stderr.trim()
    );
    Ok(())
}

/// `None` when `src` has no marketplace; malformed JSON is an error.
fn read_marketplace(src: &Path) -> anyhow::Result<Option<Marketplace>> {
    let path = src.join(".claude-plugin/marketplace.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    let Some(name) = json["name"].as_str() else {
        return Ok(None);
    };
    let plugins = json["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["name"].as_str().map(String::from))
        .collect();
    Ok(Some(Marketplace {
        name: name.into(),
        plugins,
    }))
}

/// A skill found in a source tree (offered as an install choice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableSkill {
    pub name: String,
    pub path: PathBuf,
}

/// Every skill (dir containing `SKILL.md`) under `root`, recursively, sorted by name.
/// Skips `.git` and `node_modules`; duplicate names keep the lexicographically first path.
pub fn scan_available(root: &Path) -> anyhow::Result<Vec<AvailableSkill>> {
    let mut found = Vec::new();
    walk(root, &mut found)?;
    found.sort_by(|a: &AvailableSkill, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
    found.dedup_by(|b, a| a.name == b.name);
    Ok(found)
}

fn walk(dir: &Path, found: &mut Vec<AvailableSkill>) -> anyhow::Result<()> {
    if dir.join("SKILL.md").is_file()
        && let Some(name) = dir.file_name()
    {
        found.push(AvailableSkill {
            name: name.to_string_lossy().into_owned(),
            path: dir.to_path_buf(),
        });
    }
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let skip = matches!(entry.file_name().to_str(), Some(".git" | "node_modules"));
        // file_type() does not follow symlinks, so symlink loops can't recurse forever.
        if !skip && entry.file_type()?.is_dir() {
            walk(&entry.path(), found)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::Output;
    use std::cell::RefCell;
    use std::fs;

    type Call = (String, Vec<String>, PathBuf);

    /// Records calls; `git clone` materialises `fixture` into the target dir.
    struct FakeRunner {
        calls: RefCell<Vec<Call>>,
        fail: Option<&'static str>,
        fixture: Box<dyn Fn(&Path)>,
    }

    impl FakeRunner {
        fn new(fixture: impl Fn(&Path) + 'static) -> Self {
            FakeRunner {
                calls: RefCell::default(),
                fail: None,
                fixture: Box::new(fixture),
            }
        }
        fn failing(mut self, program: &'static str) -> Self {
            self.fail = Some(program);
            self
        }
        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[String], cwd: &Path) -> anyhow::Result<Output> {
            self.calls
                .borrow_mut()
                .push((program.into(), args.to_vec(), cwd.into()));
            let success = self.fail != Some(program);
            if success && program == "git" && args.first().map(String::as_str) == Some("clone") {
                let dest = Path::new(args.last().unwrap());
                fs::create_dir_all(dest)?;
                (self.fixture)(dest);
            }
            Ok(Output {
                success,
                stdout: String::new(),
                stderr: "boom".into(),
            })
        }
    }

    struct ScriptedPrompter {
        picks: Vec<usize>,
        confirm: bool,
        offered: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptedPrompter {
        fn new(picks: &[usize], confirm: bool) -> Self {
            ScriptedPrompter {
                picks: picks.to_vec(),
                confirm,
                offered: RefCell::default(),
            }
        }
    }

    impl Prompter for ScriptedPrompter {
        fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
            self.offered.borrow_mut().push(options.to_vec());
            Ok(self.picks.clone())
        }
        fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
            Ok(self.confirm)
        }
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn skill(root: &Path, rel: &str) {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
    }

    fn skills_fixture(root: &Path) {
        for rel in [
            "skills/tdd",
            "skills/git-commit",
            "skills/caveman-commit",
            "other/zeta",
        ] {
            skill(root, rel);
        }
    }

    fn marketplace_fixture(root: &Path) {
        skill(root, "plugins/p1/skills/alpha");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"name":"mp","plugins":[{"name":"p1"},{"name":"p2"}]}"#,
        )
        .unwrap();
    }

    struct Fixture {
        _home: tempfile::TempDir,
        _project: tempfile::TempDir,
        env: Env,
    }

    /// Env whose project contains `repo/` populated by `f`.
    fn project_with(f: impl Fn(&Path)) -> Fixture {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        f(&project.path().join("repo"));
        let env = Env {
            home: home.path().into(),
            project: project.path().into(),
        };
        Fixture {
            _home: home,
            _project: project,
            env,
        }
    }

    fn req(
        source: &str,
        agents: &[Agent],
        scope: Scope,
        selection: SkillSelection,
    ) -> InstallRequest {
        InstallRequest {
            source: source.into(),
            agents: agents.to_vec(),
            scope,
            selection,
        }
    }

    fn npx(source: &str, rest: &[&str]) -> Vec<String> {
        let mut v = s(&["-y", "skills", "add", source]);
        v.extend(s(rest));
        v
    }

    #[test]
    fn local_path_all_installs_every_skill_from_absolute_path() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::All,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        let expected = npx(
            &src,
            &[
                "--skill",
                "caveman-commit",
                "--skill",
                "git-commit",
                "--skill",
                "tdd",
                "--skill",
                "zeta",
                "-a",
                "claude-code",
                "-y",
            ],
        );
        assert_eq!(
            runner.calls(),
            vec![("npx".into(), expected, fx.env.project.clone())]
        );
    }

    #[test]
    fn github_shorthand_clones_then_passes_original_source() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture);
        let r = req(
            "org/repo",
            &[Agent::Codex],
            Scope::Global,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[2], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        let (prog, args, cwd) = &calls[0];
        assert_eq!(prog, "git");
        assert_eq!(
            args[..4],
            s(&["clone", "--depth", "1", "https://github.com/org/repo.git"])
        );
        assert_eq!(cwd, &fx.env.project);
        assert!(!Path::new(&args[4]).exists(), "tmp clone removed");
        assert_eq!(
            calls[1].1,
            npx("org/repo", &["--skill", "tdd", "-a", "codex", "-g", "-y"])
        );
    }

    #[test]
    fn clone_url_keeps_full_urls() {
        assert_eq!(clone_url("https://x.dev/a/b.git"), "https://x.dev/a/b.git");
        assert_eq!(
            clone_url("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
        assert_eq!(clone_url("a/b"), "https://github.com/a/b.git");
    }

    #[test]
    fn clone_failure_is_err_and_installs_nothing() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(skills_fixture).failing("git");
        let r = req(
            "org/repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::All,
        );
        assert!(install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).is_err());
        assert_eq!(runner.calls().len(), 1);
    }

    #[test]
    fn npx_failure_is_err() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {}).failing("npx");
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::All,
        );
        assert!(install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).is_err());
    }

    #[test]
    fn patterns_confirmed_install_matches() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let sel = SkillSelection::Patterns(s(&["*commit, tdd"]));
        let r = req("repo", &[Agent::Cursor], Scope::Project, sel);
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        let expected = npx(
            &src,
            &[
                "--skill",
                "caveman-commit",
                "--skill",
                "git-commit",
                "--skill",
                "tdd",
                "-a",
                "cursor",
                "-y",
            ],
        );
        assert_eq!(runner.calls()[0].1, expected);
    }

    #[test]
    fn patterns_declined_installs_nothing() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Patterns(s(&["tdd"])),
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], false)).unwrap();
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn patterns_without_matches_is_err() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Patterns(s(&["nope*"])),
        );
        assert!(install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).is_err());
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn interactive_offers_names_and_cancel_is_ok() {
        let fx = project_with(skills_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &prompter).unwrap();
        assert_eq!(
            prompter.offered.borrow()[0],
            s(&["caveman-commit", "git-commit", "tdd", "zeta"])
        );
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn marketplace_path_adds_marketplace_and_installs_plugins() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0, 1], true);
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &prompter).unwrap();
        assert_eq!(prompter.offered.borrow()[0], s(&["p1", "p2"]));
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        let calls: Vec<Cmd> = runner.calls().into_iter().map(|(p, a, _)| (p, a)).collect();
        assert_eq!(
            calls,
            vec![
                ("claude".into(), s(&["plugin", "marketplace", "add", &src])),
                (
                    "claude".into(),
                    s(&["plugin", "install", "p1@mp", "--scope", "project"])
                ),
                (
                    "claude".into(),
                    s(&["plugin", "install", "p2@mp", "--scope", "project"])
                ),
            ]
        );
    }

    #[test]
    fn marketplace_with_other_agents_also_installs_all_skills_for_them() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude, Agent::Codex],
            Scope::Global,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[1], true)).unwrap();
        let calls = runner.calls();
        assert_eq!(
            calls[1].1,
            s(&["plugin", "install", "p2@mp", "--scope", "user"])
        );
        let src = fx.env.project.join("repo").to_string_lossy().into_owned();
        assert_eq!(
            calls[2].1,
            npx(&src, &["--skill", "alpha", "-a", "codex", "-g", "-y"])
        );
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn marketplace_cancel_installs_nothing() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Interactive,
        );
        install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).unwrap();
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn marketplace_ignored_without_claude_or_with_explicit_selection() {
        for (agents, sel) in [
            (vec![Agent::Codex], SkillSelection::Interactive),
            (vec![Agent::Claude], SkillSelection::All),
        ] {
            let fx = project_with(marketplace_fixture);
            let runner = FakeRunner::new(|_| {});
            let r = req("repo", &agents, Scope::Project, sel);
            install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[0], true)).unwrap();
            let calls = runner.calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "npx");
        }
    }

    #[test]
    fn claude_plugin_failure_is_err() {
        let fx = project_with(marketplace_fixture);
        let runner = FakeRunner::new(|_| {}).failing("claude");
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::Interactive,
        );
        assert!(install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[0], true)).is_err());
        assert_eq!(runner.calls().len(), 1);
    }

    #[test]
    fn malformed_marketplace_is_err() {
        let fx = project_with(|root| {
            fs::create_dir_all(root.join(".claude-plugin")).unwrap();
            fs::write(root.join(".claude-plugin/marketplace.json"), "{nope").unwrap();
        });
        let runner = FakeRunner::new(|_| {});
        let r = req(
            "repo",
            &[Agent::Claude],
            Scope::Project,
            SkillSelection::All,
        );
        assert!(install(&r, &fx.env, &runner, &ScriptedPrompter::new(&[], true)).is_err());
    }

    #[test]
    fn skills_command_maps_agent_ids_and_global_flag() {
        let (prog, args) = skills_command(
            "o/r",
            &s(&["a"]),
            &[Agent::Claude, Agent::Codex, Agent::Cursor],
            Scope::Global,
        );
        assert_eq!(prog, "npx");
        assert_eq!(
            args,
            npx(
                "o/r",
                &[
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
                ]
            )
        );
        let (_, project) = skills_command("o/r", &s(&["a"]), &[Agent::Codex], Scope::Project);
        assert!(!project.contains(&"-g".to_string()));
    }

    #[test]
    fn marketplace_commands_scope_flags() {
        let user = marketplace_commands("o/r", "mp", &s(&["p"]), Scope::Global);
        assert_eq!(
            user[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "user"])
        );
        let proj = marketplace_commands("o/r", "mp", &s(&["p"]), Scope::Project);
        assert_eq!(
            proj[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "project"])
        );
    }

    #[test]
    fn select_skills_splits_and_trims_patterns() {
        let names = s(&["a-commit", "b", "c"]);
        let sel = SkillSelection::Patterns(s(&[" *commit ,c", ""]));
        assert_eq!(select_skills(&names, &sel), s(&["a-commit", "c"]));
        assert_eq!(select_skills(&names, &SkillSelection::All), names);
    }

    #[test]
    fn scan_available_recurses_skips_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        skill(root, "b/deep/nested/beta");
        skill(root, "a/alpha");
        skill(root, "z/alpha");
        skill(root, ".git/hidden");
        skill(root, "node_modules/pkg/dep");
        let found = scan_available(root).unwrap();
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta"]);
        assert_eq!(found[0].path, root.join("a/alpha"));
        assert_eq!(found[1].path, root.join("b/deep/nested/beta"));
    }

    #[test]
    fn scan_available_root_itself_can_be_a_skill() {
        let dir = tempfile::tempdir().unwrap();
        skill(dir.path(), "solo");
        let found = scan_available(&dir.path().join("solo")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "solo");
    }

    #[test]
    fn redact_hides_url_credentials_only() {
        assert!(!redact("https://u:tok@github.com/o/r.git").contains("tok"));
        assert_eq!(
            redact("https://github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
        assert_eq!(redact("git@github.com:o/r.git"), "git@github.com:o/r.git");
        assert_eq!(redact("https://github.com/a@b"), "https://github.com/a@b");
    }
}
