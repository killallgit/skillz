use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List profiles and how each compares with the project
    #[command(visible_alias = "ls")]
    List {
        /// Print the profiles with their comparison as JSON
        #[arg(long)]
        json: bool,
    },
    /// What the project has and lacks of a profile, and what it has beyond it (default: every profile)
    Status {
        name: Option<String>,
        /// Print the comparison as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print a profile as TOML, to read or copy (`export`)
    #[command(visible_alias = "export")]
    Show { name: String },
    /// Store the profile in a TOML file printed by `show`, replacing one of the same name
    Import { file: PathBuf },
    /// Create an empty profile
    New { name: String },
    /// Add entries to a profile: one typed as a line, or copies of library items
    Add(ProfileAddArgs),
    /// Take entries out of a profile (the profile itself stays; see `delete`)
    Remove(ProfileRemoveArgs),
    /// Save what is installed in the project as a profile
    Save {
        name: String,
        /// Replace a profile of that name
        #[arg(long)]
        force: bool,
    },
    /// Delete a profile (what it installed stays until you `use` another)
    #[command(visible_alias = "rm")]
    Delete { name: String },
}

#[derive(Debug, Args)]
pub struct ProfileAddArgs {
    pub name: String,
    /// The entry as one line, quoted: `skill o/r tdd`, `hook Stop fmt`, ...
    #[arg(allow_hyphen_values = true, required_unless_present = "from_library")]
    pub spec: Option<String>,
    /// Fuzzy query over the library's labels; repeat for more items
    #[arg(long = "from-library", value_name = "QUERY")]
    pub from_library: Vec<String>,
    /// Don't prompt; each query must match one item
    #[arg(short, long)]
    pub yes: bool,
    /// Say what would be added without changing the profile
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ProfileRemoveArgs {
    pub name: String,
    /// Fuzzy query over the profile's entries
    pub query: String,
    /// Don't prompt; the query must match one entry
    #[arg(short, long)]
    pub yes: bool,
    /// Say what would be removed without changing the profile
    #[arg(long)]
    pub dry_run: bool,
}
