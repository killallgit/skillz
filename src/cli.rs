use std::collections::{BTreeSet, HashSet};
use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};

use crate::action::{self, Action, Edit};
use crate::agents::{self, AgentAdapter};
use crate::doctor;
use crate::env::Env;
use crate::install::{self, InstallRequest, SkillSelection};
use crate::matcher;
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::prompt::{AssumeYes, Choice, Prompter, TuiPrompter};
use crate::runner::{CommandRunner, SystemRunner};
use crate::tree;
use crate::usage;

#[derive(Debug, Parser)]
#[command(
    name = "skillz",
    version,
    about = "Manage skills, plugins, marketplaces and hooks for Claude, Codex and Cursor"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show installed augmentations as a tree (groups collapsed unless queried or --all)
    #[command(visible_alias = "ls")]
    List {
        /// Fuzzy query to expand, or a project path (`.`, `./dir`, `/abs`) to limit the listing to
        target: Option<String>,
        /// Project directory (default: cwd + global)
        #[arg(short = 'C', long)]
        project: Option<PathBuf>,
        /// Expand every group
        #[arg(long)]
        all: bool,
        /// Print the matching items (with their removal plans) as JSON, no tree
        #[arg(long, conflicts_with = "interactive")]
        json: bool,
        #[command(flatten)]
        filter: Filter,
        /// Browse in a collapsible tree
        #[arg(short, long)]
        interactive: bool,
    },
    /// Completely remove augmentations (fuzzy match, then pick)
    #[command(visible_aliases = ["rm", "delete"])]
    Uninstall {
        /// Fuzzy query over name, group and source
        query: Option<String>,
        /// Project directory (default: cwd)
        #[arg(short = 'C', long)]
        project: Option<PathBuf>,
        #[command(flatten)]
        filter: Filter,
        /// Don't prompt; requires an unambiguous query
        #[arg(short, long)]
        yes: bool,
        /// Print the removal plan without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Find duplicate skills and skills/plugins unused for 7+ days, then remove the ones picked
    Doctor {
        /// Project directory (default: cwd)
        #[arg(short = 'C', long)]
        project: Option<PathBuf>,
        #[command(flatten)]
        filter: Filter,
        /// Print the findings and their removal plan without changing anything
        #[arg(long, conflicts_with = "force")]
        dry_run: bool,
        /// Remove everything found without asking
        #[arg(short, long)]
        force: bool,
    },
    /// Install skills (via `npx skills`) or a Claude plugin marketplace
    #[command(visible_alias = "add")]
    Install {
        /// org/repo, git URL or local path
        source: String,
        /// Install globally instead of into the project
        #[arg(short, long)]
        global: bool,
        /// Target agents (default: all)
        #[arg(short, long = "agent", value_enum)]
        agents: Vec<Agent>,
        /// Install every skill in the source
        #[arg(long, conflicts_with = "skill")]
        all: bool,
        /// Skill names or globs, comma separated (e.g. `tdd,*commit`)
        #[arg(long, value_delimiter = ',')]
        skill: Vec<String>,
        /// Project directory (default: cwd)
        #[arg(short = 'C', long)]
        project: Option<PathBuf>,
        /// Don't prompt; pick everything matched
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Args, Default, Clone)]
pub struct Filter {
    /// Global scope only
    #[arg(short, long)]
    pub global: bool,
    #[arg(long, visible_alias = "skill")]
    pub skills: bool,
    #[arg(long, visible_alias = "hook")]
    pub hooks: bool,
    #[arg(long, visible_alias = "plugin")]
    pub plugins: bool,
    #[arg(long, visible_alias = "marketplace")]
    pub marketplaces: bool,
    /// Only items used by these agents
    #[arg(short, long = "agent", value_enum)]
    pub agents: Vec<Agent>,
}

impl Filter {
    fn kinds(&self) -> Vec<Kind> {
        [
            (self.skills, Kind::Skill),
            (self.hooks, Kind::Hook),
            (self.plugins, Kind::Plugin),
            (self.marketplaces, Kind::Marketplace),
        ]
        .into_iter()
        .filter_map(|(on, kind)| on.then_some(kind))
        .collect()
    }

    pub fn keeps(&self, item: &Augmentation) -> bool {
        let kinds = self.kinds();
        (kinds.is_empty() || kinds.contains(&item.kind))
            && (self.agents.is_empty() || item.agents.iter().any(|a| self.agents.contains(a)))
    }
}

/// External effects the commands need; real ones in `main`, fakes in tests.
pub struct Deps<'a> {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub adapters: Vec<Box<dyn AgentAdapter>>,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
    pub interactive: bool,
}

pub fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let prompter: &dyn Prompter = if wants_auto_yes(&cli.command) {
        &AssumeYes
    } else {
        &TuiPrompter
    };
    let deps = Deps {
        home: std::env::home_dir().context("cannot find home directory")?,
        cwd: std::env::current_dir()?,
        adapters: agents::registry(),
        runner: &SystemRunner,
        prompter,
        // Pickers and the browser read keys from stdin and draw on stdout.
        interactive: std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
    };
    run(cli.command, &deps, &mut std::io::stdout())
}

/// `--yes` (doctor: `--force`) swaps the interactive prompter for one that picks and confirms everything.
fn wants_auto_yes(command: &Command) -> bool {
    matches!(
        command,
        Command::Uninstall { yes: true, .. }
            | Command::Install { yes: true, .. }
            | Command::Doctor { force: true, .. }
    )
}

pub fn run(command: Command, deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    match command {
        Command::List {
            target,
            project,
            all,
            json,
            filter,
            interactive,
        } => {
            let (path, query) = split_target(project, target);
            let (env, scopes) = resolve(deps, path.as_deref(), filter.global)?;
            let view = ListView {
                query: query.as_deref(),
                all,
                json,
                interactive,
            };
            list(deps, &env, &scopes, &filter, view, out)
        }
        Command::Uninstall {
            query,
            project,
            filter,
            yes,
            dry_run,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), filter.global)?;
            uninstall(
                deps,
                &env,
                &scopes,
                &filter,
                query.as_deref(),
                yes,
                dry_run,
                out,
            )
        }
        Command::Doctor {
            project,
            filter,
            dry_run,
            force: _,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), filter.global)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
            doctor(
                deps,
                &env,
                &scopes,
                &filter,
                now.as_millis() as u64,
                dry_run,
                out,
            )
        }
        Command::Install {
            source,
            global,
            agents,
            all,
            skill,
            project,
            yes: _,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let req = InstallRequest {
                source,
                cwd: deps.cwd.clone(),
                agents: if agents.is_empty() {
                    vec![Agent::Claude, Agent::Codex, Agent::Cursor]
                } else {
                    agents
                },
                // In the home dir (no project scope left) the project is the global one.
                scope: if global || !scopes.contains(&Scope::Project) {
                    Scope::Global
                } else {
                    Scope::Project
                },
                selection: match (all, skill.is_empty()) {
                    (true, _) => SkillSelection::All,
                    (false, false) => SkillSelection::Patterns(skill),
                    (false, true) => SkillSelection::Interactive,
                },
            };
            install::install(&req, &env, deps.runner, deps.prompter)
        }
    }
}

/// Scope rule: a path alone means that project, `-g` alone means global, neither means both.
fn resolve(deps: &Deps, path: Option<&Path>, global: bool) -> anyhow::Result<(Env, Vec<Scope>)> {
    let project = match path {
        Some(p) => deps.cwd.join(p),
        None => deps.cwd.clone(),
    };
    let project = project
        .canonicalize()
        .with_context(|| format!("no such directory: {}", project.display()))?;
    let home = deps
        .home
        .canonicalize()
        .unwrap_or_else(|_| deps.home.clone());
    // In the home dir "project" files are the global ones, so that project is global scope.
    let scopes = match (project == home, path.is_some(), global) {
        (true, _, _) | (false, false, true) => vec![Scope::Global],
        (false, true, false) => vec![Scope::Project],
        _ => vec![Scope::Global, Scope::Project],
    };
    Ok((Env { home, project }, scopes))
}

fn gather(deps: &Deps, env: &Env, scopes: &[Scope], filter: &Filter) -> Vec<Augmentation> {
    let inv = agents::inventory(&deps.adapters, env, scopes);
    for warning in &inv.warnings {
        eprintln!("warning: {warning}");
    }
    inv.items.into_iter().filter(|i| filter.keeps(i)).collect()
}

/// `list TARGET`: a path starting with `.`, `..` or `/` is the project (as in
/// `list --hooks .`); anything else, even a word naming a directory, is a query. With `-C`,
/// TARGET is always a query.
fn split_target(
    project: Option<PathBuf>,
    target: Option<String>,
) -> (Option<PathBuf>, Option<String>) {
    let path_like = |t: &str| {
        matches!(
            Path::new(t).components().next(),
            Some(Component::CurDir | Component::ParentDir | Component::RootDir)
        )
    };
    match (project, target) {
        (None, Some(t)) if path_like(&t) => (Some(PathBuf::from(t)), None),
        other => other,
    }
}

struct ListView<'a> {
    query: Option<&'a str>,
    all: bool,
    json: bool,
    interactive: bool,
}

/// Items `list QUERY` shows: the matches, plus the whole group of any matched marketplace.
fn queried(items: &[Augmentation], hits: &[&Augmentation]) -> Vec<Augmentation> {
    let ids: HashSet<String> = hits.iter().map(|i| i.id.clone()).collect();
    let groups: HashSet<String> = hits
        .iter()
        .filter(|i| i.kind == Kind::Marketplace)
        .map(|i| i.group.clone())
        .collect();
    items
        .iter()
        .filter(|i| ids.contains(&i.id) || groups.contains(&i.group))
        .cloned()
        .collect()
}

/// Browse `items` in the tree UI, then show or remove whatever the user marks.
fn browse(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    items: &[Augmentation],
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let nodes: Vec<tree::Node> = tree::build(items)
        .into_iter()
        .map(|mut node| {
            node.label = format!("{}  ({})", node.label, node.children.len());
            node
        })
        .collect();
    let Some((choice, picked)) = deps.prompter.browse("skillz", &nodes)? else {
        return Ok(());
    };
    // `tree::build` makes one node per group, sorted by group.
    let groups: Vec<&str> = items
        .iter()
        .map(|i| i.group.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let chosen: Vec<&Augmentation> = items
        .iter()
        .enumerate()
        .filter(|(i, item)| {
            picked.items.contains(i)
                || picked
                    .groups
                    .iter()
                    .any(|&g| groups.get(g) == Some(&item.group.as_str()))
        })
        .map(|(_, item)| item)
        .collect();
    match choice {
        Choice::Show => {
            write!(
                out,
                "{}",
                tree::render(&tree::build(&queried(items, &chosen)))
            )?;
            Ok(())
        }
        Choice::Remove => remove(deps, env, scopes, filter, &chosen, false, out),
    }
}

fn list(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    view: ListView,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let mut items = gather(deps, env, scopes, filter);
    if let Some(q) = view.query {
        items = queried(&items, &matches(&items, Some(q)));
    }
    if view.json {
        serde_json::to_writer_pretty(&mut *out, &items)?;
        writeln!(out)?;
        return Ok(());
    }
    // A query spanning several groups opens the browser so the user can narrow it down.
    let several_groups = items.iter().any(|i| i.group != items[0].group);
    if deps.interactive && (view.interactive || (view.query.is_some() && several_groups)) {
        return browse(deps, env, scopes, filter, &items, out);
    }
    let nodes = tree::build(&items);
    if items.is_empty() {
        let msg = if view.query.is_some() {
            "nothing installed matches"
        } else {
            "nothing installed"
        };
        writeln!(out, "{msg}")?;
    }
    let expand = view.all || view.query.is_some();
    let nodes = if expand { nodes } else { tree::collapse(nodes) };
    write!(out, "{}", tree::render(&nodes))?;
    Ok(())
}

/// Items named exactly `query` (case-insensitive, surrounding whitespace ignored).
fn named<'a>(items: &'a [Augmentation], query: &str) -> Vec<&'a Augmentation> {
    items
        .iter()
        .filter(|i| i.name.eq_ignore_ascii_case(query.trim()))
        .collect()
}

/// Items matching `query`: exact (case-insensitive) name matches win over fuzzy ones.
pub fn matches<'a>(items: &'a [Augmentation], query: Option<&str>) -> Vec<&'a Augmentation> {
    let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
        return items.iter().collect();
    };
    let exact = named(items, query);
    if !exact.is_empty() {
        return exact;
    }
    let haystacks: Vec<String> = items
        .iter()
        .map(|i| {
            format!(
                "{} {} {}",
                i.name,
                i.group,
                i.source.as_deref().unwrap_or("")
            )
        })
        .collect();
    matcher::fuzzy_rank(query, &haystacks)
        .into_iter()
        .map(|i| &items[i])
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn uninstall(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    query: Option<&str>,
    yes: bool,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = gather(deps, env, scopes, filter);
    let candidates = matches(&items, query);
    if candidates.is_empty() {
        bail!("nothing installed matches");
    }
    // `--yes` selects every candidate, so it only acts on exactly one exact-name match.
    if yes && query.map_or(0, |q| named(&items, q).len()) != 1 {
        bail!(
            "{} items match; refine the query or run without --yes to pick",
            candidates.len()
        );
    }
    let labels: Vec<String> = candidates.iter().map(|c| c.label(true)).collect();
    // A dry run changes nothing, so show every candidate's plan without asking.
    let picked = if dry_run {
        (0..candidates.len()).collect()
    } else {
        deps.prompter
            .multi_select("Select items to remove", &labels)?
    };
    let chosen: Vec<&Augmentation> = picked
        .into_iter()
        .filter_map(|i| candidates.get(i).copied())
        .collect();
    remove(deps, env, scopes, filter, &chosen, dry_run, out)
}

/// Report findings; duplicates are always in the removal, unused items only when picked.
fn doctor(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    now_ms: u64,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = gather(deps, env, scopes, filter);
    let usage = usage::read(env)?;
    let found = doctor::check(&items, &usage, now_ms);
    let unused: Vec<&Augmentation> = found
        .never_used
        .iter()
        .copied()
        .chain(found.unused.iter().map(|(i, _)| *i))
        .collect();
    if found.duplicates.is_empty() && unused.is_empty() {
        writeln!(out, "nothing to clean up")?;
        return Ok(());
    }
    let days_ago = |ms: u64| now_ms.saturating_sub(ms) / (24 * 60 * 60 * 1000);
    let sections: [(&str, Vec<String>); 3] = [
        (
            "Duplicates (also provided by a plugin)",
            found.duplicates.iter().map(|i| i.label(true)).collect(),
        ),
        (
            "Never used",
            found.never_used.iter().map(|i| i.label(true)).collect(),
        ),
        (
            "Unused for 7+ days",
            found
                .unused
                .iter()
                .map(|(i, ms)| format!("{}  last used {}d ago", i.label(true), days_ago(*ms)))
                .collect(),
        ),
    ];
    for (title, lines) in sections.iter().filter(|(_, l)| !l.is_empty()) {
        writeln!(out, "{title}:")?;
        for line in lines {
            writeln!(out, "  {line}")?;
        }
    }
    let mut chosen = found.duplicates.clone();
    if dry_run {
        chosen.extend(&unused);
    } else if !unused.is_empty() {
        let labels: Vec<String> = unused.iter().map(|i| i.label(true)).collect();
        let picked = deps
            .prompter
            .multi_select("Select unused items to remove", &labels)?;
        chosen.extend(picked.into_iter().filter_map(|i| unused.get(i).copied()));
    }
    remove(deps, env, scopes, filter, &chosen, dry_run, out)
}

/// Print the removal plan for `chosen`, confirm, execute, then verify they are gone.
fn remove(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    chosen: &[&Augmentation],
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    if chosen.is_empty() {
        writeln!(out, "nothing selected")?;
        return Ok(());
    }

    let plan: Vec<Action> = chosen.iter().flat_map(|c| c.removal.clone()).collect();
    writeln!(out, "Removing:")?;
    for c in chosen {
        writeln!(out, "  {}", c.label(true))?;
        // Removal is always complete, so say when `-a` narrows the view but not the effect.
        let others: Vec<String> = c
            .agents
            .iter()
            .filter(|a| !filter.agents.is_empty() && !filter.agents.contains(a))
            .map(Agent::to_string)
            .collect();
        if !others.is_empty() {
            writeln!(out, "    also removes it for: {}", others.join(", "))?;
        }
    }
    writeln!(out, "Plan:")?;
    for step in &plan {
        writeln!(out, "  {}", describe(step))?;
    }
    if dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }

    let report = action::execute(&plan)?;
    writeln!(
        out,
        "removed {} paths, edited {} files",
        report.removed.len(),
        report.edited.len()
    )?;

    let after = agents::inventory(&deps.adapters, env, scopes);
    let left: Vec<&str> = chosen
        .iter()
        .filter(|c| after.items.iter().any(|i| i.id == c.id))
        .map(|c| c.name.as_str())
        .collect();
    if !left.is_empty() {
        bail!("still installed after removal: {}", left.join(", "));
    }
    Ok(())
}

fn describe(step: &Action) -> String {
    match step {
        Action::RemovePath(t) => format!("rm {}", t.path().display()),
        Action::RemovePathIfJsonAbsent { target, file, key } => format!(
            "rm {} (if {} then has no {})",
            target.path().display(),
            file.display(),
            key.join(".")
        ),
        Action::Edit { file, edit } => {
            let what = match edit {
                Edit::RemoveHook { event, command } => format!("hook {event}: {command}"),
                Edit::JsonRemoveKey { path } | Edit::TomlRemoveKey { path } => path.join("."),
                Edit::JsonRemoveArrayItems { path, fields } => {
                    let when: Vec<String> =
                        fields.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    format!("{}[{}]", path.join("."), when.join(","))
                }
            };
            format!("edit {} (remove {what})", file.display())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps_at(home: &Path, cwd: &Path) -> Deps<'static> {
        Deps {
            home: home.to_path_buf(),
            cwd: cwd.to_path_buf(),
            adapters: vec![],
            runner: &SystemRunner,
            prompter: &AssumeYes,
            interactive: false,
        }
    }

    fn scopes(deps: &Deps, path: Option<&str>, global: bool) -> Vec<Scope> {
        resolve(deps, path.map(Path::new), global).unwrap().1
    }

    #[test]
    fn resolve_home_as_project_is_global() {
        let home = tempfile::tempdir().unwrap();
        let deps = deps_at(home.path(), home.path());
        assert_eq!(scopes(&deps, None, false), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("."), false), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("."), true), [Scope::Global]);
    }

    #[test]
    fn resolve_scope_rule_outside_home() {
        let (home, cwd) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        std::fs::create_dir(cwd.path().join("sub")).unwrap();
        let deps = deps_at(home.path(), cwd.path());
        let both = [Scope::Global, Scope::Project];
        assert_eq!(scopes(&deps, None, false), both);
        assert_eq!(scopes(&deps, None, true), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("sub"), false), [Scope::Project]);
        assert_eq!(scopes(&deps, Some("sub"), true), both);
        let (env, _) = resolve(&deps, Some(Path::new("sub")), false).unwrap();
        assert_eq!(env.project, cwd.path().join("sub").canonicalize().unwrap());
        assert!(resolve(&deps, Some(Path::new("missing")), false).is_err());
    }

    #[test]
    fn auto_yes_only_for_yes_and_force_flags() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from([&["skillz"], args].concat())
                .unwrap()
                .command
        };
        assert!(wants_auto_yes(&parse(&["rm", "x", "-y"])));
        assert!(!wants_auto_yes(&parse(&["rm", "x"])));
        assert!(wants_auto_yes(&parse(&["install", "o/r", "--yes"])));
        assert!(!wants_auto_yes(&parse(&["install", "o/r"])));
        assert!(!wants_auto_yes(&parse(&["list"])));
        assert!(wants_auto_yes(&parse(&["doctor", "--force"])));
        assert!(!wants_auto_yes(&parse(&["doctor"])));
    }

    fn split(project: Option<&str>, target: &str) -> (Option<PathBuf>, Option<String>) {
        split_target(project.map(PathBuf::from), Some(target.into()))
    }

    #[test]
    fn split_target_path_like_is_project() {
        for t in [".", "..", "./sub", "../sib", "/abs/dir"] {
            assert_eq!(split(None, t), (Some(PathBuf::from(t)), None), "{t}");
        }
    }

    #[test]
    fn split_target_word_is_query() {
        for t in ["caveman", "org/repo", ".hidden"] {
            assert_eq!(split(None, t), (None, Some(t.into())), "{t}");
        }
        assert_eq!(
            split(Some("p"), "."),
            (Some(PathBuf::from("p")), Some(".".into()))
        );
    }
}
