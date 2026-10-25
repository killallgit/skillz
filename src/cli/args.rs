use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use super::library_args::LibraryCommand;
use super::profile_args::ProfileCommand;
use skillz_harness::model::{Agent, Installed, Kind};

#[derive(Debug, Parser)]
#[command(
    name = "skillz",
    version,
    about = "Manage skills, plugins, marketplaces and hooks for Claude and Codex"
)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Without one, opens the TUI
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Directory the TUI opens on (default: cwd)
    pub path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show installed items as a tree (groups collapsed unless queried or --all)
    #[command(visible_alias = "ls")]
    List(ListArgs),
    /// Completely remove installed items (fuzzy match, then pick)
    #[command(visible_aliases = ["rm", "delete"])]
    Uninstall(UninstallArgs),
    /// Update installed items with the tool that installed them (`--check` only lists the ones with a newer version)
    Update(UpdateArgs),
    /// Remove every skill, plugin and marketplace in scope (hooks, MCP servers and subagents only when asked: `--hooks`, `--mcp`, `--subagents`)
    Reset(ResetArgs),
    /// Find duplicate skills and skills/plugins unused for 7+ days, then remove the ones picked
    Prune(PruneArgs),
    /// Install what a project lacks of a profile (with --exact, remove the rest)
    Use(UseArgs),
    /// Create, inspect and delete profiles
    Profile {
        #[command(subcommand)]
        action: ProfileCommand,
        /// Project directory (default: cwd)
        #[arg(short = 'p', long, global = true)]
        project: Option<PathBuf>,
    },
    /// List, add to, remove from and install from the library of available items
    #[command(visible_alias = "lib")]
    Library {
        #[command(subcommand)]
        action: LibraryCommand,
        /// Project directory (default: cwd)
        #[arg(short = 'p', long, global = true)]
        project: Option<PathBuf>,
    },
    /// Run a command (say `claude`) in a config dir of its own that holds just one profile
    Run(RunArgs),
    /// Add everything installed, in the project and at user level, to the library; never removes
    Sync(SyncArgs),
    /// Install skills (via `npx skills`) or a Claude plugin marketplace
    #[command(visible_alias = "add")]
    Install(InstallArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Fuzzy query to expand, or a project path (`.`, `./dir`, `/abs`) to limit the listing to
    pub target: Option<String>,
    /// Project directory (default: cwd + user-level installs)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Expand every group
    #[arg(long)]
    pub all: bool,
    /// Print the matching items (with their location and removal plans) as JSON, no tree
    #[arg(long, conflicts_with = "interactive")]
    pub json: bool,
    /// With --json, say which items have a newer version (needs the network)
    #[arg(long, requires = "json")]
    pub check: bool,
    /// List what the library offers that the project (with -g: the user) does not have installed
    #[arg(long, conflicts_with_all = ["interactive", "all", "check"])]
    pub available: bool,
    #[command(flatten)]
    pub filter: Filter,
    /// Browse in a collapsible tree
    #[arg(short, long)]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Profile to load into its own isolated Claude and Codex config dirs
    pub profile: String,
    /// Project directory to run in (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Don't ask before changing the profile's config dirs
    #[arg(short, long)]
    pub yes: bool,
    /// The command and its arguments, after `--`
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Fuzzy queries over name, group and source; the items of all of them go in one plan
    pub queries: Vec<String>,
    /// Project directory (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    #[command(flatten)]
    pub filter: Filter,
    /// Don't prompt; requires every query to be unambiguous
    #[arg(short, long)]
    pub yes: bool,
    /// Print the removal plan without changing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Fuzzy query over name, group and source, or a project path (`.`, `./dir`, `/abs`)
    pub target: Option<String>,
    /// Project directory (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    #[command(flatten)]
    pub filter: Filter,
    /// Don't prompt; requires an unambiguous query
    #[arg(short, long)]
    pub yes: bool,
    /// Print the update plan without changing anything
    #[arg(long, conflicts_with = "check")]
    pub dry_run: bool,
    /// List the items with a newer version available instead of updating
    #[arg(long)]
    pub check: bool,
    /// With --check, print the items as JSON
    #[arg(long, requires = "check")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ResetArgs {
    /// Project directory (default: the current one). With `-g`, only global unless given.
    #[arg(value_name = "PATH")]
    pub project: Option<PathBuf>,
    #[command(flatten)]
    pub filter: Filter,
    /// Spare items with this name, group or source (repeatable)
    #[arg(long)]
    pub keep: Vec<String>,
    /// Print what would be removed without changing anything
    #[arg(long, conflicts_with = "force")]
    pub dry_run: bool,
    /// Remove without asking
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Project directory (default: the current one, plus user-level installs)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Only what is installed at user level
    #[arg(short, long)]
    pub global: bool,
    /// Say what would be added without changing the library
    #[arg(long)]
    pub dry_run: bool,
    /// Print what was added and what was left out as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PruneArgs {
    /// Project directory (default: the current one, plus user-level installs)
    #[arg(value_name = "PATH")]
    pub project: Option<PathBuf>,
    #[command(flatten)]
    pub filter: Filter,
    /// Print the findings and their removal plan without changing anything
    #[arg(long, conflicts_with = "force")]
    pub dry_run: bool,
    /// Remove everything found without asking
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct UseArgs {
    /// Profile name (see `skillz profile list`)
    pub profile: String,
    /// Also remove what the profile does not want
    #[arg(long)]
    pub exact: bool,
    /// Project directory (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Don't ask before changing anything
    #[arg(short, long)]
    pub yes: bool,
    /// Print the plan without changing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// org/repo, git URL or local path
    pub source: String,
    /// Target agents (default: all)
    #[arg(short, long = "agent", value_enum)]
    pub agents: Vec<Agent>,
    /// Install every skill in the source
    #[arg(long, conflicts_with = "skill")]
    pub all: bool,
    /// Skill names or globs, comma separated (e.g. `tdd,*commit`)
    #[arg(long, value_delimiter = ',')]
    pub skill: Vec<String>,
    /// Project directory (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Install for the user instead of the project
    #[arg(short, long, conflicts_with = "project")]
    pub global: bool,
    /// Don't prompt; pick everything matched
    #[arg(short, long)]
    pub yes: bool,
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
    #[arg(long)]
    pub mcp: bool,
    #[arg(long, visible_alias = "subagent")]
    pub subagents: bool,
    /// Only items used by these agents
    #[arg(short, long = "agent", value_enum)]
    pub agents: Vec<Agent>,
}

impl Filter {
    pub(super) fn kinds(&self) -> Vec<Kind> {
        [
            (self.skills, Kind::Skill),
            (self.hooks, Kind::Hook),
            (self.plugins, Kind::Plugin),
            (self.marketplaces, Kind::Marketplace),
            (self.mcp, Kind::Mcp),
            (self.subagents, Kind::Subagent),
        ]
        .into_iter()
        .filter_map(|(on, kind)| on.then_some(kind))
        .collect()
    }

    pub fn keeps(&self, item: &Installed) -> bool {
        let kinds = self.kinds();
        (kinds.is_empty() || kinds.contains(&item.kind))
            && (self.agents.is_empty() || item.agents.iter().any(|a| self.agents.contains(a)))
    }
}

/// `--yes` (prune: `--force`) swaps the interactive prompter for one that picks and confirms everything.
pub(super) fn wants_auto_yes(command: &Command) -> bool {
    match command {
        Command::Uninstall(a) => a.yes,
        Command::Install(a) => a.yes,
        Command::Update(a) => a.yes,
        Command::Use(a) => a.yes,
        Command::Run(a) => a.yes,
        Command::Library {
            action: LibraryCommand::Remove(a),
            ..
        } => a.yes,
        Command::Library {
            action: LibraryCommand::Install(a),
            ..
        } => a.yes,
        Command::Profile {
            action: ProfileCommand::Add(a),
            ..
        } => a.yes,
        Command::Profile {
            action: ProfileCommand::Remove(a),
            ..
        } => a.yes,
        Command::Prune(a) => a.force,
        Command::Reset(a) => a.force,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_tree_is_valid_and_profile_commands_parse() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
        for args in [
            &["profile", "save", "x", "-p", "."][..],
            &["profile", "rm", "x"],
            &["use", "x", "--dry-run"],
            &["use", "x", "--exact"],
            &["sync", "-g", "--dry-run", "--json"],
            &["sync", "-p", "."],
            &["library", "ls", "--json", "--skills"],
            &["library", "add", "hook Stop --x"],
            &["library", "remove", "tdd", "-y", "--dry-run"],
            &["library", "install", "a", "b", "-g", "-y"],
            &["profile", "add", "web", "--from-library", "tdd"],
            &["profile", "remove", "web", "tdd"],
            &["profile", "status", "--json"],
            &["list", "--available", "-g", "--json"],
            &["list", "--json", "--check"],
        ] {
            assert!(
                Cli::try_parse_from([&["skillz"], args].concat()).is_ok(),
                "{args:?}"
            );
        }
        assert!(Cli::try_parse_from(["skillz", "use"]).is_err());
        assert!(Cli::try_parse_from(["skillz", "update", "tdd", "-y", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["skillz", "update", "--check", "--json"]).is_ok());
        assert!(Cli::try_parse_from(["skillz", "update", "--json"]).is_err());
        assert!(Cli::try_parse_from(["skillz", "update", "--check", "--dry-run"]).is_err());
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
            Command::Reset(a) => a.project,
            Command::Prune(a) => a.project,
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

    #[test]
    fn a_bare_path_opens_the_tui_there_and_a_subcommand_still_wins() {
        let bare = Cli::try_parse_from(["skillz", "some/dir"]).unwrap();
        let none = Cli::try_parse_from(["skillz"]).unwrap();
        let listing = Cli::try_parse_from(["skillz", "list"]).unwrap();

        assert_eq!(bare.path, Some(PathBuf::from("some/dir")));
        assert!(bare.command.is_none() && none.path.is_none());
        assert!(listing.command.is_some() && listing.path.is_none());
    }
}
