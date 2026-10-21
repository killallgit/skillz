use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::model::{Agent, Augmentation, Kind};

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
    List(ListArgs),
    /// Completely remove augmentations (fuzzy match, then pick)
    #[command(visible_aliases = ["rm", "delete"])]
    Uninstall(UninstallArgs),
    /// Remove every skill, plugin and marketplace in scope (hooks, MCP servers and subagents only when asked: `--hooks`, `--mcp`, `--subagents`)
    Reset(ResetArgs),
    /// Find duplicate skills and skills/plugins unused for 7+ days, then remove the ones picked
    Prune(PruneArgs),
    /// Load a profile into the project (or globally), swapping out the one that is there
    Use(UseArgs),
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
    /// Run a command (say `claude`) in a config dir of its own that holds just one profile
    Run(RunArgs),
    /// Install skills (via `npx skills`) or a Claude plugin marketplace
    #[command(visible_alias = "add")]
    Install(InstallArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Fuzzy query to expand, or a project path (`.`, `./dir`, `/abs`) to limit the listing to
    pub target: Option<String>,
    /// Project directory (default: cwd + global)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    /// Expand every group
    #[arg(long)]
    pub all: bool,
    /// Print the matching items (with their removal plans) as JSON, no tree
    #[arg(long, conflicts_with = "interactive")]
    pub json: bool,
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
    /// Fuzzy query over name, group and source
    pub query: Option<String>,
    /// Project directory (default: cwd)
    #[arg(short = 'p', long)]
    pub project: Option<PathBuf>,
    #[command(flatten)]
    pub filter: Filter,
    /// Don't prompt; requires an unambiguous query
    #[arg(short, long)]
    pub yes: bool,
    /// Print the removal plan without changing anything
    #[arg(long)]
    pub dry_run: bool,
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
pub struct PruneArgs {
    /// Project directory (default: the current one, plus global)
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
    /// Profile name (see `~/.config/skillz/profiles`)
    #[arg(required_unless_present = "none", conflicts_with = "none")]
    pub profile: Option<String>,
    /// Unload the current profile instead
    #[arg(long)]
    pub none: bool,
    /// Apply to the global config instead of the project
    #[arg(short, long)]
    pub global: bool,
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
    /// Install globally instead of into the project
    #[arg(short, long)]
    pub global: bool,
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
    /// Don't prompt; pick everything matched
    #[arg(short, long)]
    pub yes: bool,
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

    pub fn keeps(&self, item: &Augmentation) -> bool {
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
        Command::Use(a) => a.yes,
        Command::Run(a) => a.yes,
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
}
