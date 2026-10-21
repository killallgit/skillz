use std::collections::HashSet;
use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};

use crate::agents::{self, AgentAdapter};
use crate::apply;
use crate::cost;
use crate::env::Env;
use crate::install::{self, InstallRequest, SkillSelection};
use crate::model::{Agent, Augmentation, Kind, Scope};
use crate::profile;
use crate::prompt::{AssumeYes, Choice, Prompter, TuiPrompter};
use crate::prune;
use crate::runner::{CommandRunner, SystemRunner};
use crate::snapshot;
use crate::tree;
use crate::tui::home;
use crate::usage;
use crate::workspace::Workspace;
use crate::{info, remove, spinner};

#[derive(Debug, Parser)]
#[command(
    name = "skillz",
    version,
    about = "Manage skills, plugins, marketplaces and hooks for Claude, Codex and Cursor"
)]
pub struct Cli {
    /// Without one, opens the workspace
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show installed augmentations as a tree (groups collapsed unless queried or --all)
    #[command(visible_alias = "ls")]
    List {
        /// Fuzzy query to expand, or a project path (`.`, `./dir`, `/abs`) to limit the listing to
        target: Option<String>,
        /// Project directory (default: cwd + global)
        #[arg(short = 'p', long)]
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
        #[arg(short = 'p', long)]
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
    /// Remove every skill, plugin and marketplace in scope (hooks only with `--hooks`)
    Reset {
        /// Project directory (default: the current one). With `-g`, only global unless given.
        #[arg(value_name = "PATH")]
        project: Option<PathBuf>,
        #[command(flatten)]
        filter: Filter,
        /// Spare items with this name, group or source (repeatable)
        #[arg(long)]
        keep: Vec<String>,
        /// Print what would be removed without changing anything
        #[arg(long, conflicts_with = "force")]
        dry_run: bool,
        /// Remove without asking
        #[arg(short, long)]
        force: bool,
    },
    /// Find duplicate skills and skills/plugins unused for 7+ days, then remove the ones picked
    Prune {
        /// Project directory (default: the current one, plus global)
        #[arg(value_name = "PATH")]
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
    /// Load a profile into the project (or globally), swapping out the one that is there
    Use {
        /// Profile name (see `~/.config/skillz/profiles`)
        #[arg(required_unless_present = "none", conflicts_with = "none")]
        profile: Option<String>,
        /// Unload the current profile instead
        #[arg(long)]
        none: bool,
        /// Apply to the global config instead of the project
        #[arg(short, long)]
        global: bool,
        /// Project directory (default: cwd)
        #[arg(short = 'p', long)]
        project: Option<PathBuf>,
        /// Don't ask before changing anything
        #[arg(short, long)]
        yes: bool,
        /// Print the plan without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Register projects so the workspace can show and switch them
    Project {
        #[command(subcommand)]
        action: ProjectCommand,
    },
    /// Create, inspect and delete profiles
    Profile {
        #[command(subcommand)]
        action: ProfileCommand,
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
        #[arg(short = 'p', long)]
        project: Option<PathBuf>,
        /// Don't prompt; pick everything matched
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List registered projects and the profile each has loaded
    #[command(visible_alias = "ls")]
    List,
    /// Register a project (default: the current directory)
    Add { path: Option<PathBuf> },
    /// Forget a project (nothing installed there changes)
    #[command(visible_alias = "rm")]
    Remove { path: PathBuf },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List profiles
    #[command(visible_alias = "ls")]
    List,
    /// Print a profile's file
    Show { name: String },
    /// Save what is installed in the project (or globally) as a profile
    Save {
        name: String,
        /// Read the global config instead of the project
        #[arg(short, long)]
        global: bool,
        /// Project directory (default: cwd)
        #[arg(short = 'p', long)]
        project: Option<PathBuf>,
        /// Replace a profile of that name
        #[arg(long)]
        force: bool,
    },
    /// Delete a profile (what it installed stays until you `use` another)
    #[command(visible_alias = "rm")]
    Delete { name: String },
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
    /// `CLAUDE_CONFIG_DIR`, when set: the only Claude config skillz touches.
    pub claude_config: Option<PathBuf>,
    /// `CODEX_HOME`, when set: the only Codex config skillz touches.
    pub codex_home: Option<PathBuf>,
    pub cwd: PathBuf,
    pub adapters: Vec<Box<dyn AgentAdapter>>,
    pub runner: &'a dyn CommandRunner,
    pub prompter: &'a dyn Prompter,
    pub interactive: bool,
}

pub fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let prompter: &dyn Prompter = if cli.command.as_ref().is_some_and(wants_auto_yes) {
        &AssumeYes
    } else {
        &TuiPrompter
    };
    let dir_var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let claude_config = dir_var("CLAUDE_CONFIG_DIR");
    let codex_home = dir_var("CODEX_HOME");
    let runner = SystemRunner {
        envs: [
            ("CLAUDE_CONFIG_DIR", &claude_config),
            ("CODEX_HOME", &codex_home),
        ]
        .into_iter()
        .filter_map(|(k, v)| Some((k, v.clone()?)))
        .collect(),
    };
    let deps = Deps {
        home: std::env::home_dir().context("cannot find home directory")?,
        claude_config,
        codex_home,
        cwd: std::env::current_dir()?,
        adapters: agents::registry(),
        runner: &runner,
        prompter,
        // Pickers and the browser read keys from stdin and draw on stdout.
        interactive: std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
    };
    match cli.command {
        Some(command) => run(command, &deps, &mut std::io::stdout()),
        None => workspace(&deps, &mut std::io::stdout()),
    }
}

/// `--yes` (prune: `--force`) swaps the interactive prompter for one that picks and confirms everything.
fn wants_auto_yes(command: &Command) -> bool {
    matches!(
        command,
        Command::Uninstall { yes: true, .. }
            | Command::Install { yes: true, .. }
            | Command::Prune { force: true, .. }
            | Command::Reset { force: true, .. }
            | Command::Use { yes: true, .. }
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
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            uninstall(&scene, query.as_deref(), yes, dry_run, out)
        }
        Command::Prune {
            project,
            filter,
            dry_run,
            force: _,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), filter.global)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            prune(&scene, now.as_millis() as u64, dry_run, out)
        }
        Command::Reset {
            project,
            filter,
            keep,
            dry_run,
            force: _,
        } => {
            // Destructive, so the project you are in is the default; `-g` alone is global.
            let path = project.or_else(|| (!filter.global).then(|| PathBuf::from(".")));
            let (env, scopes) = resolve(deps, path.as_deref(), filter.global)?;
            let scene = Scene {
                deps,
                env: &env,
                scopes: &scopes,
                filter: &filter,
            };
            reset(&scene, &keep, dry_run, out)
        }
        Command::Use {
            profile,
            none: _,
            global,
            project,
            yes: _,
            dry_run,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let scope = single_scope(&scopes, global);
            use_profile(deps, &env, scope, profile.as_deref(), dry_run, out)
        }
        Command::Profile { action } => profile_command(deps, action, out),
        Command::Project { action } => project_command(deps, action, out),
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
                    agents::all()
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
            install::install(&deps.adapters, &req, &env, deps.runner, deps.prompter)
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
    let claude_config = deps.claude_config.clone();
    let codex_home = deps.codex_home.clone();
    Ok((
        Env {
            home,
            project,
            claude_config,
            codex_home,
        },
        scopes,
    ))
}

fn gather(deps: &Deps, env: &Env, scopes: &[Scope], filter: &Filter) -> Vec<Augmentation> {
    let inv = agents::inventory(&deps.adapters, env, scopes);
    for warning in &inv.warnings {
        eprintln!("warning: {warning}");
    }
    inv.items.into_iter().filter(|i| filter.keeps(i)).collect()
}

/// `list TARGET`: a path starting with `.`, `..` or `/` is the project (as in
/// `list --hooks .`); anything else, even a word naming a directory, is a query. With `-p`,
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

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `tree::build` plus each row's info-modal text. Last-use data is optional: if an agent's
/// records cannot be read the modal just omits it.
fn display_tree(deps: &Deps, env: &Env, items: &[Augmentation]) -> Vec<tree::Node> {
    let usage = usage::read(&deps.adapters, env).ok();
    let now = now_ms();
    let mut nodes = tree::build(items);
    tree::attach_info(&mut nodes, &|i| {
        let item = &items[i];
        let used = match (&usage, prune::tracked(item, &deps.adapters)) {
            (Some(u), true) => u.last_used(item).map_or(info::Used::Never, info::Used::At),
            _ => info::Used::NotTracked,
        };
        info::item_lines(item, used, now)
    });
    nodes
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
    let nodes = display_tree(deps, env, items);
    let Some((choice, picked)) = deps.prompter.browse("skillz", &nodes)? else {
        return Ok(());
    };
    let chosen: Vec<&Augmentation> = picked
        .item_indices(&nodes)
        .into_iter()
        .filter_map(|i| items.get(i))
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
        Choice::Remove => {
            let scene = Scene {
                deps,
                env,
                scopes,
                filter,
            };
            confirm_and_remove(&scene, &chosen, false, true, out)
        }
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
        items = queried(&items, &remove::matches(&items, Some(q)));
    }
    if view.json {
        // Read the costs only here: the report takes a second or so (`claude -p /context`).
        let costs = cost::read(&deps.adapters, env, deps.runner);
        let rows: Vec<serde_json::Value> = items
            .iter()
            .map(|i| {
                let mut row = serde_json::to_value(i)?;
                row["tokens"] = serde_json::json!(costs.tokens(i));
                anyhow::Ok(row)
            })
            .collect::<Result<_, _>>()?;
        serde_json::to_writer_pretty(&mut *out, &rows)?;
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

/// What the removal flows share: where to look and what the filters allow.
struct Scene<'a, 'd> {
    deps: &'a Deps<'d>,
    env: &'a Env,
    scopes: &'a [Scope],
    filter: &'a Filter,
}

impl Scene<'_, '_> {
    fn gather(&self) -> Vec<Augmentation> {
        gather(self.deps, self.env, self.scopes, self.filter)
    }
}

fn uninstall(
    scene: &Scene,
    query: Option<&str>,
    yes: bool,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.gather();
    let candidates = remove::candidates(&items, query, yes)?;
    // A dry run changes nothing, so show every candidate's plan without asking.
    let picked = if dry_run {
        (0..candidates.len()).collect()
    } else {
        let owned: Vec<Augmentation> = candidates.iter().map(|&c| c.clone()).collect();
        scene.deps.prompter.select_items(
            "Select items to remove",
            &display_tree(scene.deps, scene.env, &owned),
        )?
    };
    let chosen: Vec<&Augmentation> = picked
        .into_iter()
        .filter_map(|i| candidates.get(i).copied())
        .collect();
    confirm_and_remove(scene, &chosen, dry_run, true, out)
}

/// Remove everything the filters leave, except what `keep` names. Hooks belong to other tools
/// (rtk, beads), so they go only when asked for with `--hooks`.
fn reset(scene: &Scene, keep: &[String], dry_run: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    let items = scene.gather();
    let chosen = reset_set(&items, scene.filter, keep);
    confirm_and_remove(scene, &chosen, dry_run, true, out)
}

fn reset_set<'a>(
    items: &'a [Augmentation],
    filter: &Filter,
    keep: &[String],
) -> Vec<&'a Augmentation> {
    let named = |i: &Augmentation, q: &String| {
        [
            Some(i.name.as_str()),
            Some(i.group.as_str()),
            i.source.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|f| f.eq_ignore_ascii_case(q.trim()))
    };
    let spared: Vec<&Augmentation> = items
        .iter()
        .filter(|i| keep.iter().any(|q| named(i, q)))
        .collect();
    // A marketplace takes its plugins with it, so it stays while any of them is spared.
    let spared_groups: Vec<&str> = spared.iter().map(|i| i.group.as_str()).collect();
    let explicit_kinds = !filter.kinds().is_empty();
    items
        .iter()
        .filter(|i| filter.keeps(i))
        .filter(|i| explicit_kinds || i.kind != Kind::Hook)
        .filter(|i| !spared.contains(i))
        .filter(|i| !(i.kind == Kind::Marketplace && spared_groups.contains(&i.name.as_str())))
        .collect()
}

/// Report findings; duplicates are always in the removal, unused items only when picked.
fn prune(scene: &Scene, now_ms: u64, dry_run: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    let (deps, env) = (scene.deps, scene.env);
    let items = scene.gather();
    let usage = usage::read(&deps.adapters, env)?;
    let found = prune::check(&items, &deps.adapters, &usage, now_ms);
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
        let owned: Vec<Augmentation> = unused.iter().map(|&i| i.clone()).collect();
        let picked = deps.prompter.select_items(
            "Select unused items to remove",
            &display_tree(deps, env, &owned),
        )?;
        chosen.extend(picked.into_iter().filter_map(|i| unused.get(i).copied()));
    }
    // A dry run already listed every item, with its age, in the sections above.
    confirm_and_remove(scene, &chosen, dry_run, !dry_run, out)
}

/// Show the removal plan for `chosen` (and the items themselves if `list_items`), confirm,
/// run it, and report what it did.
fn confirm_and_remove(
    scene: &Scene,
    chosen: &[&Augmentation],
    dry_run: bool,
    list_items: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let deps = scene.deps;
    if chosen.is_empty() {
        writeln!(out, "nothing selected")?;
        return Ok(());
    }

    let plan = remove::Plan::new(chosen, &scene.filter.agents);
    if list_items {
        writeln!(out, "Removing:")?;
        for c in chosen {
            writeln!(out, "  {}", c.label(true))?;
            if let Some((_, others)) = plan.also_removes.iter().find(|(i, _)| i.id == c.id) {
                let others: Vec<String> = others.iter().map(Agent::to_string).collect();
                writeln!(out, "    also removes it for: {}", others.join(", "))?;
            }
        }
    }
    writeln!(out, "Plan:")?;
    for step in &plan.steps {
        writeln!(out, "  {}", step.describe())?;
    }
    if dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }

    // One status line while it works: the thing being removed, not each command's chatter.
    let spinner = spinner::Spinner::start("removing");
    let result = remove::run(
        &plan,
        &deps.adapters,
        scene.env,
        scene.scopes,
        deps.runner,
        &mut |step| {
            if let Some(s) = &spinner {
                s.set(&format!("removing {}", plan.owner(step)));
            }
        },
    );
    drop(spinner);
    let outcome = result?;
    writeln!(
        out,
        "ran {} commands, removed {} paths, edited {} files",
        outcome.report.ran.len(),
        outcome.report.removed.len(),
        outcome.report.edited.len()
    )?;
    if !outcome.still_installed.is_empty() {
        bail!(
            "still installed after removal: {}",
            outcome.still_installed.join(", ")
        );
    }
    Ok(())
}

/// One scope at a time: the project, unless asked for global or this is the home dir.
fn single_scope(scopes: &[Scope], global: bool) -> Scope {
    if global || !scopes.contains(&Scope::Project) {
        Scope::Global
    } else {
        Scope::Project
    }
}

fn project_command(deps: &Deps, action: ProjectCommand, out: &mut dyn Write) -> anyhow::Result<()> {
    let home_env = Env {
        claude_config: deps.claude_config.clone(),
        codex_home: deps.codex_home.clone(),
        ..Env::new(deps.home.clone(), deps.cwd.clone())
    };
    let workspace = Workspace::in_env(&home_env);
    match action {
        ProjectCommand::List => {
            let ledger = apply::Ledger::in_env(&home_env);
            for dir in workspace.projects()? {
                let env = Env {
                    project: dir.clone(),
                    ..home_env.clone()
                };
                let loaded = ledger.get(&apply::Ledger::key(&env, Scope::Project))?;
                let state = loaded.map_or("-".to_string(), |r| r.profile);
                let gone = if dir.is_dir() { "" } else { "  (missing)" };
                writeln!(out, "{state:<16} {}{gone}", dir.display())?;
            }
        }
        ProjectCommand::Add { path } => {
            let dir = deps.cwd.join(path.unwrap_or_else(|| PathBuf::from(".")));
            if workspace.add(&dir)? {
                writeln!(out, "added {}", dir.canonicalize()?.display())?;
            }
        }
        ProjectCommand::Remove { path } => {
            if !workspace.remove(&deps.cwd.join(path))? {
                bail!("not a registered project");
            }
        }
    }
    Ok(())
}

fn profile_command(deps: &Deps, action: ProfileCommand, out: &mut dyn Write) -> anyhow::Result<()> {
    let home_env = Env::new(deps.home.clone(), deps.cwd.clone());
    let store = profile::Store::in_env(&home_env);
    match action {
        ProfileCommand::List => {
            for name in store.names()? {
                match store.load(&name) {
                    Ok(p) => writeln!(out, "{name}  ({} entries)", p.entries.len())?,
                    Err(e) => writeln!(out, "{name}  (unreadable: {e:#})")?,
                }
            }
        }
        ProfileCommand::Show { name } => write!(out, "{}", store.load(&name)?.to_text()?)?,
        ProfileCommand::Delete { name } => {
            store.load(&name)?;
            store.delete(&name)?;
        }
        ProfileCommand::Save {
            name,
            global,
            project,
            force,
        } => {
            let (env, scopes) = resolve(deps, project.as_deref(), global)?;
            let scope = single_scope(&scopes, global);
            let snap = save_profile(deps, &store, &env, scope, &name, force)?;
            writeln!(out, "saved {name} from the {scope} config:")?;
            for entry in &snap.profile.entries {
                writeln!(out, "  {entry}")?;
            }
            for label in &snap.skipped {
                writeln!(out, "  not saved (nothing to reinstall it from): {label}")?;
            }
        }
    }
    Ok(())
}

/// Snapshot what `scope` of `env` has installed into the profile `name`.
fn save_profile(
    deps: &Deps,
    store: &profile::Store,
    env: &Env,
    scope: Scope,
    name: &str,
    force: bool,
) -> anyhow::Result<snapshot::Snapshot> {
    profile::check_name(name)?;
    if !force && store.names()?.iter().any(|n| n == name) {
        bail!("profile {name} exists; use --force to replace it");
    }
    let items = gather(deps, env, &[scope], &Filter::default());
    let snap = snapshot::snapshot(name, &items);
    store.save(&snap.profile)?;
    Ok(snap)
}

/// The environment of one registered project, with the config dirs this run redirects.
fn project_env(deps: &Deps, dir: &Path) -> Env {
    Env {
        project: dir.to_path_buf(),
        claude_config: deps.claude_config.clone(),
        codex_home: deps.codex_home.clone(),
        ..Env::new(deps.home.clone(), dir.to_path_buf())
    }
}

fn workspace_model(
    deps: &Deps,
    store: &profile::Store,
    workspace: &Workspace,
    notice: Option<String>,
) -> anyhow::Result<home::Model> {
    let ledger = apply::Ledger::in_env(&project_env(deps, &deps.cwd));
    let short = |p: &Path| match p.strip_prefix(&deps.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    };
    let projects = workspace
        .projects()?
        .into_iter()
        .map(|path| {
            let env = project_env(deps, &path);
            let record = ledger
                .get(&apply::Ledger::key(&env, Scope::Project))
                .ok()
                .flatten();
            let missing = !path.is_dir();
            let profile_name = record.as_ref().map_or("-", |r| r.profile.as_str());
            let mut detail = vec![path.display().to_string()];
            if missing {
                detail.push("directory is missing".into());
            }
            for loaded in record.iter().flat_map(|r| &r.entries) {
                detail.push(format!("{}", loaded.entry));
                detail.extend(loaded.items.iter().map(|o| format!("    {}", o.name)));
            }
            home::ProjectRow {
                label: format!(
                    "{profile_name:<14} {}{}",
                    short(&path),
                    if missing { "  (missing)" } else { "" }
                ),
                loaded: record.is_some(),
                missing,
                detail,
                path,
            }
        })
        .collect();
    let profiles = store
        .names()?
        .into_iter()
        .map(|name| match store.load(&name) {
            Ok(p) => home::ProfileRow {
                detail: p.entries.iter().map(ToString::to_string).collect(),
                broken: false,
                name,
            },
            Err(e) => home::ProfileRow {
                detail: vec![format!("{e:#}")],
                broken: true,
                name,
            },
        })
        .collect();
    Ok(home::Model {
        projects,
        profiles,
        notice,
    })
}

/// What the workspace does after a step.
enum Flow {
    Stay,
    Leave,
}

/// The workspace: pick a project and a profile and apply, unload, add, save or delete, with the
/// plan and progress of each change shown as for `use`. A step that fails shows its error in the
/// footer and the workspace stays open.
pub fn workspace(deps: &Deps, out: &mut dyn Write) -> anyhow::Result<()> {
    let base = project_env(deps, &deps.cwd);
    let store = profile::Store::in_env(&base);
    let registry = Workspace::in_env(&base);
    let mut pos = home::Pos::default();
    let mut notice = None;
    loop {
        let model = workspace_model(deps, &store, &registry, notice.take())?;
        let (act, next) = deps.prompter.home(model, pos)?;
        pos = next;
        match workspace_step(deps, &store, &registry, act, &mut notice, out) {
            Ok(Flow::Leave) => return Ok(()),
            Ok(Flow::Stay) => {}
            Err(e) => notice = Some(format!("{e:#}")),
        }
    }
}

fn workspace_step(
    deps: &Deps,
    store: &profile::Store,
    registry: &Workspace,
    act: home::Act,
    notice: &mut Option<String>,
    out: &mut dyn Write,
) -> anyhow::Result<Flow> {
    use home::Act;
    let projects = registry.projects()?;
    let names = store.names()?;
    let switch =
        |project: usize, name: Option<&str>, out: &mut dyn Write| -> anyhow::Result<Flow> {
            let Some(dir) = projects.get(project) else {
                return Ok(Flow::Stay);
            };
            use_profile(
                deps,
                &project_env(deps, dir),
                Scope::Project,
                name,
                false,
                out,
            )?;
            out.flush()?;
            Ok(if deps.prompter.confirm("Back to the workspace?")? {
                Flow::Stay
            } else {
                Flow::Leave
            })
        };
    match act {
        Act::Quit => return Ok(Flow::Leave),
        Act::Apply { project, profile } => {
            let name = names.get(profile).map(String::as_str);
            return switch(project, name, out);
        }
        Act::Unload { project } => return switch(project, None, out),
        Act::AddProject(text) => {
            registry.add(&deps.cwd.join(text))?;
        }
        Act::ForgetProject(i) => {
            if let Some(dir) = projects.get(i) {
                registry.remove(dir)?;
            }
        }
        Act::NewProfile(name) => {
            profile::check_name(&name)?;
            anyhow::ensure!(!names.contains(&name), "profile {name} exists");
            store.save(&profile::Profile::new(&name))?;
        }
        Act::SaveProfile { name, project } => {
            if let Some(dir) = projects.get(project) {
                let env = project_env(deps, dir);
                let snap = save_profile(deps, store, &env, Scope::Project, &name, false)?;
                *notice = Some(format!(
                    "saved {name}: {} entries, {} not reproducible",
                    snap.profile.entries.len(),
                    snap.skipped.len()
                ));
            }
        }
        Act::DeleteProfile(i) => {
            if let Some(name) = names.get(i) {
                store.delete(name)?;
            }
        }
    }
    Ok(Flow::Stay)
}

/// `use`: show what moving `scope` to the named profile (`None` = no profile) changes, ask, then
/// do it with one status line.
fn use_profile(
    deps: &Deps,
    env: &Env,
    scope: Scope,
    name: Option<&str>,
    dry_run: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let ledger = apply::Ledger::in_env(env);
    let target = name
        .map(|n| profile::Store::in_env(env).load(n))
        .transpose()?;
    let switch = apply::plan(&deps.adapters, target.as_ref(), scope, env, &ledger)?;

    let label = |p: &Option<String>| p.clone().unwrap_or_else(|| "none".into());
    writeln!(
        out,
        "{scope}: {} -> {}",
        label(&switch.from),
        label(&switch.to)
    )?;
    for l in &switch.keep {
        writeln!(out, "  keep    {}", l.entry)?;
    }
    for l in &switch.drop {
        writeln!(out, "  remove  {}", l.entry)?;
    }
    for (entry, _) in &switch.installs {
        writeln!(out, "  install {entry}")?;
    }
    for u in &switch.unsupported {
        writeln!(
            out,
            "  skip    {} for {}: not supported at {scope} scope",
            u.entry, u.agent
        )?;
    }
    if switch.is_empty() {
        writeln!(out, "already up to date")?;
        return Ok(());
    }
    writeln!(out, "Plan:")?;
    for step in switch.steps() {
        writeln!(out, "  {}", step.describe())?;
    }
    if dry_run {
        return Ok(());
    }
    if !deps.prompter.confirm("Proceed?")? {
        writeln!(out, "aborted")?;
        return Ok(());
    }

    let spinner = spinner::Spinner::start("switching");
    let refs: Vec<&Augmentation> = switch.removals.iter().collect();
    let removal = remove::Plan::new(&refs, &[]);
    let result = apply::run(
        &deps.adapters,
        &switch,
        scope,
        env,
        &ledger,
        deps.runner,
        &mut |step| {
            let Some(s) = &spinner else { return };
            let installing = switch
                .installs
                .iter()
                .find(|(_, steps)| steps.contains(step));
            match installing {
                Some((entry, _)) => s.set(&format!("installing {entry}")),
                None => s.set(&format!("removing {}", removal.owner(step))),
            }
        },
    );
    drop(spinner);
    let done = result?;
    if scope == Scope::Project {
        // Remembering the project is a convenience; the switch itself already worked.
        let _ = Workspace::in_env(env).add(&env.project);
    }
    writeln!(
        out,
        "removed {} items, installed {} items",
        switch.removals.len(),
        done.added.len()
    )?;
    if let Some(left) = done.removed.filter(|o| !o.still_installed.is_empty()) {
        bail!(
            "still installed after removal: {}",
            left.still_installed.join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static NO_ENVS: SystemRunner = SystemRunner { envs: Vec::new() };

    fn deps_at(home: &Path, cwd: &Path) -> Deps<'static> {
        Deps {
            home: home.to_path_buf(),
            claude_config: None,
            codex_home: None,
            cwd: cwd.to_path_buf(),
            adapters: vec![],
            runner: &NO_ENVS,
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
    fn the_command_tree_is_valid_and_profile_commands_parse() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
        for args in [
            &["profile", "save", "x", "-g"][..],
            &["profile", "rm", "x"],
            &["use", "x", "--dry-run"],
            &["use", "--none"],
        ] {
            assert!(
                Cli::try_parse_from([&["skillz"], args].concat()).is_ok(),
                "{args:?}"
            );
        }
        assert!(Cli::try_parse_from(["skillz", "use"]).is_err());
        assert!(Cli::try_parse_from(["skillz", "use", "x", "--none"]).is_err());
    }

    #[test]
    fn auto_yes_only_for_yes_and_force_flags() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from([&["skillz"], args].concat())
                .unwrap()
                .command
                .unwrap()
        };
        assert!(wants_auto_yes(&parse(&["rm", "x", "-y"])));
        assert!(!wants_auto_yes(&parse(&["rm", "x"])));
        assert!(wants_auto_yes(&parse(&["install", "o/r", "--yes"])));
        assert!(!wants_auto_yes(&parse(&["install", "o/r"])));
        assert!(!wants_auto_yes(&parse(&["list"])));
        assert!(wants_auto_yes(&parse(&["prune", "--force"])));
        assert!(!wants_auto_yes(&parse(&["prune"])));
        assert!(wants_auto_yes(&parse(&["reset", "-f"])));
        assert!(!wants_auto_yes(&parse(&["reset"])));
        assert!(Cli::try_parse_from(["skillz", "reset", "--dry-run", "--force"]).is_err());
        let project = |args: &[&str]| match parse(args) {
            Command::Reset { project, .. } | Command::Prune { project, .. } => project,
            other => panic!("{other:?}"),
        };
        assert_eq!(project(&["reset", "."]), Some(PathBuf::from(".")));
        assert_eq!(project(&["reset", "../x"]), Some(PathBuf::from("../x")));
        assert_eq!(project(&["reset"]), None);
        assert_eq!(
            project(&["prune", "/p", "--dry-run"]),
            Some(PathBuf::from("/p"))
        );
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

    fn item(name: &str, kind: Kind, group: &str) -> Augmentation {
        Augmentation {
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            group: group.into(),
            source: None,
            installer: crate::model::Installer::Local,
            children: vec![],
            removal: vec![],
        }
    }

    #[test]
    fn reset_skips_hooks_unless_asked_and_spares_kept_groups_with_their_marketplace() {
        let items = vec![
            item("tdd", Kind::Skill, "local"),
            item("rtk", Kind::Hook, "hooks"),
            item("p@mp", Kind::Plugin, "mp"),
            item("mp", Kind::Marketplace, "mp"),
            item("other@om", Kind::Plugin, "om"),
            item("om", Kind::Marketplace, "om"),
        ];
        let names = |set: Vec<&Augmentation>| -> Vec<String> {
            set.iter().map(|i| i.name.clone()).collect()
        };
        let all = reset_set(&items, &Filter::default(), &[]);
        assert_eq!(names(all), ["tdd", "p@mp", "mp", "other@om", "om"]);
        let hooks = Filter {
            hooks: true,
            ..Filter::default()
        };
        assert_eq!(names(reset_set(&items, &hooks, &[])), ["rtk"]);
        let kept = reset_set(&items, &Filter::default(), &["MP".to_string()]);
        assert_eq!(names(kept), ["tdd", "other@om", "om"]);
        let one = reset_set(&items, &Filter::default(), &["tdd".to_string()]);
        assert_eq!(one.len(), 4);
    }
}
