use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};

use crate::action::{self, Action, Edit};
use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::install::{self, InstallRequest, SkillSelection};
use crate::matcher;
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::prompt::{AssumeYes, Prompter, TuiPrompter};
use crate::runner::{CommandRunner, SystemRunner};
use crate::tree;
use crate::tui;

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
    /// Show installed augmentations as a tree
    #[command(visible_alias = "ls")]
    List {
        /// Project directory; limits the listing to that project (default: cwd + global)
        path: Option<PathBuf>,
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
    let yes = matches!(
        cli.command,
        Command::Uninstall { yes: true, .. } | Command::Install { yes: true, .. }
    );
    let prompter: &dyn Prompter = if yes { &AssumeYes } else { &TuiPrompter };
    let deps = Deps {
        home: std::env::home_dir().context("cannot find home directory")?,
        cwd: std::env::current_dir()?,
        adapters: agents::registry(),
        runner: &SystemRunner,
        prompter,
        interactive: std::io::stdout().is_terminal(),
    };
    run(cli.command, &deps, &mut std::io::stdout())
}

pub fn run(command: Command, deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    match command {
        Command::List {
            path,
            filter,
            interactive,
        } => {
            let (env, scopes) = resolve(deps, path.as_deref(), filter.global)?;
            list(deps, &env, &scopes, &filter, interactive, out)
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
        Command::Install {
            source,
            global,
            agents,
            all,
            skill,
            project,
            yes: _,
        } => {
            let (env, _) = resolve(deps, project.as_deref(), global)?;
            let req = InstallRequest {
                source,
                agents: if agents.is_empty() {
                    vec![Agent::Claude, Agent::Codex, Agent::Cursor]
                } else {
                    agents
                },
                scope: if global {
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
    let mut scopes = match (path.is_some(), global) {
        (true, false) => vec![Scope::Project],
        (false, true) => vec![Scope::Global],
        _ => vec![Scope::Global, Scope::Project],
    };
    // In the home dir "project" files are the global ones; don't list them twice.
    if project == home {
        scopes.retain(|s| *s == Scope::Global);
    }
    Ok((Env { home, project }, scopes))
}

fn gather(deps: &Deps, env: &Env, scopes: &[Scope], filter: &Filter) -> Vec<Augmentation> {
    let inv = agents::inventory(&deps.adapters, env, scopes);
    for warning in &inv.warnings {
        eprintln!("warning: {warning}");
    }
    inv.items.into_iter().filter(|i| filter.keeps(i)).collect()
}

fn list(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
    interactive: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = gather(deps, env, scopes, filter);
    let nodes = tree::build(&items);
    if interactive && deps.interactive {
        return tui::tree_view::run("skillz", &nodes);
    }
    if items.is_empty() {
        writeln!(out, "nothing installed")?;
    }
    write!(out, "{}", tree::render(&nodes))?;
    Ok(())
}

/// Picker label for one item.
pub fn label(item: &Augmentation) -> String {
    let agents: Vec<String> = item.agents.iter().map(Agent::to_string).collect();
    let source = item
        .source
        .as_deref()
        .filter(|s| *s != item.name)
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default();
    format!(
        "{}{source}  ({} · {} · {})",
        item.name,
        item.kind,
        agents.join(","),
        item.scope
    )
}

/// Items matching `query`: exact (case-insensitive) name matches win over fuzzy ones.
pub fn matches<'a>(items: &'a [Augmentation], query: Option<&str>) -> Vec<&'a Augmentation> {
    let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
        return items.iter().collect();
    };
    let exact: Vec<&Augmentation> = items
        .iter()
        .filter(|i| i.name.eq_ignore_ascii_case(query.trim()))
        .collect();
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
    let exact = |c: &&Augmentation| query.is_some_and(|q| c.name.eq_ignore_ascii_case(q.trim()));
    if yes && (candidates.len() > 1 || !candidates.iter().all(exact)) {
        bail!(
            "{} items match; refine the query or run without --yes to pick",
            candidates.len()
        );
    }
    let labels: Vec<String> = candidates.iter().map(|c| label(c)).collect();
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
    if chosen.is_empty() {
        writeln!(out, "nothing selected")?;
        return Ok(());
    }

    let plan: Vec<Action> = chosen.iter().flat_map(|c| c.removal.clone()).collect();
    writeln!(out, "Removing:")?;
    for c in &chosen {
        writeln!(out, "  {}", label(c))?;
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
        Action::RemovePath(p) => format!("rm {}", p.display()),
        Action::Edit { file, edit } => {
            let what = match edit {
                Edit::JsonRemoveHook { event, command }
                | Edit::TomlRemoveHook { event, command } => format!("hook {event}: {command}"),
                Edit::JsonRemoveKey { path } | Edit::TomlRemoveKey { path } => path.join("."),
                Edit::JsonRemovePluginInstall { id, .. } => format!("install record {id}"),
            };
            format!("edit {} (remove {what})", file.display())
        }
    }
}
