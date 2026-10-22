use clap::{Args, Subcommand};

use crate::library::ItemKind;

#[derive(Debug, Subcommand)]
pub enum LibraryCommand {
    /// List the library: where each item came from and where it is installed
    #[command(visible_alias = "ls")]
    List(LibraryListArgs),
    /// Add an item typed as one line, as in the workspace: `skill o/r tdd`, `plugin o/r mp name`,
    /// `hook Stop fmt`, `mcp name cmd|url [args]`
    Add(AddArgs),
    /// Remove an item from the library (what it installed stays); by label or by the line that
    /// adds it
    #[command(visible_alias = "rm")]
    Remove(LibraryRemoveArgs),
    /// Install library items into the project, or with `-g` for the user
    Install(LibraryInstallArgs),
}

#[derive(Debug, Args)]
pub struct LibraryListArgs {
    #[command(flatten)]
    pub kinds: KindFlags,
    /// Print the items with their origin and where they are installed as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args, Default)]
pub struct KindFlags {
    #[arg(long, visible_alias = "skill")]
    pub skills: bool,
    #[arg(long, visible_alias = "plugin")]
    pub plugins: bool,
    #[arg(long)]
    pub mcp: bool,
    #[arg(long, visible_alias = "hook")]
    pub hooks: bool,
    #[arg(long, visible_alias = "subagent")]
    pub subagents: bool,
}

impl KindFlags {
    pub fn kinds(&self) -> Vec<ItemKind> {
        [
            (self.skills, ItemKind::Skill),
            (self.plugins, ItemKind::Plugin),
            (self.mcp, ItemKind::Mcp),
            (self.hooks, ItemKind::Hook),
            (self.subagents, ItemKind::Subagent),
        ]
        .into_iter()
        .filter_map(|(on, kind)| on.then_some(kind))
        .collect()
    }
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// The line describing the item, quoted
    #[arg(allow_hyphen_values = true)]
    pub spec: String,
    /// Check the line and say what would be added without changing the library
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct LibraryRemoveArgs {
    /// Fuzzy query over the labels, or the line that adds the item
    pub item: String,
    /// Don't prompt; requires an unambiguous query
    #[arg(short, long)]
    pub yes: bool,
    /// Say what would be removed without changing the library
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct LibraryInstallArgs {
    /// Fuzzy queries over the labels, one per item to install
    #[arg(required = true)]
    pub queries: Vec<String>,
    /// Install for the user instead of the project
    #[arg(short, long)]
    pub global: bool,
    /// Don't prompt; each query must match one item
    #[arg(short, long)]
    pub yes: bool,
    /// Print the plan without changing anything
    #[arg(long)]
    pub dry_run: bool,
}
