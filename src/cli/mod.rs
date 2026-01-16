//! CLI definitions and entry point.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub mod commands;

/// Agent-first issue tracker (`SQLite` + JSONL)
#[derive(Parser, Debug)]
#[command(name = "br", author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Database path (auto-discover .beads/*.db if not set)
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    /// Actor name for audit trail
    #[arg(long, global = true)]
    pub actor: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Force direct mode (no daemon) - effectively no-op in br v1
    #[arg(long, global = true)]
    pub no_daemon: bool,

    /// Skip auto JSONL export
    #[arg(long, global = true)]
    pub no_auto_flush: bool,

    /// Skip auto import check
    #[arg(long, global = true)]
    pub no_auto_import: bool,

    /// Allow stale DB (bypass freshness check warning)
    #[arg(long, global = true)]
    pub allow_stale: bool,

    /// `SQLite` busy timeout in ms
    #[arg(long, global = true)]
    pub lock_timeout: Option<u64>,

    /// JSONL-only mode (no DB connection)
    #[arg(long, global = true)]
    pub no_db: bool,

    /// Increase logging verbosity (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Quiet mode (no output except errors)
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a beads workspace
    Init {
        /// Issue ID prefix (e.g., "bd")
        #[arg(long)]
        prefix: Option<String>,

        /// Overwrite existing DB
        #[arg(long)]
        force: bool,

        /// Backend type (ignored, always sqlite)
        #[arg(long)]
        backend: Option<String>,
    },

    /// Create a new issue
    Create(CreateArgs),

    /// Quick capture (create issue, print ID only)
    Q(QuickArgs),

    /// List issues
    List(ListArgs),

    /// Show issue details
    Show {
        /// Issue IDs
        ids: Vec<String>,
    },

    /// Update an issue
    Update {
        /// Issue IDs
        ids: Vec<String>,
    },

    /// Close an issue
    Close {
        /// Issue IDs
        ids: Vec<String>,
    },

    /// Reopen an issue
    Reopen {
        /// Issue IDs
        ids: Vec<String>,
    },

    /// Delete an issue (creates tombstone)
    Delete(DeleteArgs),

    /// List ready issues
    Ready,

    /// List blocked issues
    Blocked,

    /// Search issues
    Search(SearchArgs),

    /// Manage dependencies
    Dep {
        #[command(subcommand)]
        command: DepCommands,
    },

    /// Manage labels
    Label {
        #[command(subcommand)]
        command: LabelCommands,
    },

    /// Manage comments
    Comments {
        #[command(subcommand)]
        command: CommentCommands,
    },

    /// Show project statistics
    Stats,

    /// Alias for stats
    Status,

    /// Count issues with optional grouping
    Count(CountArgs),

    /// Configuration management
    Config,

    /// Sync with JSONL
    Sync {
        #[arg(long)]
        flush_only: bool,
        #[arg(long)]
        import_only: bool,
    },

    /// Run read-only diagnostics
    Doctor,

    /// Show version information
    Version,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Issue title
    pub title: Option<String>,

    /// Issue title (alternative flag)
    #[arg(long)]
    pub title_flag: Option<String>, // Handled in logic

    /// Issue type (task, bug, feature, etc.)
    #[arg(long = "type", short = 't')]
    pub type_: Option<String>,

    /// Priority (0-4 or P0-P4)
    #[arg(long, short = 'p')]
    pub priority: Option<String>,

    /// Description
    #[arg(long, short = 'd')]
    pub description: Option<String>,
}

#[derive(Args, Debug)]
pub struct QuickArgs {
    /// Issue title words
    pub title: Vec<String>,

    /// Priority (0-4 or P0-P4)
    #[arg(long, short = 'p')]
    pub priority: Option<String>,

    /// Issue type (task, bug, feature, etc.)
    #[arg(long = "type", short = 't')]
    pub type_: Option<String>,

    /// Labels to apply (repeatable, comma-separated allowed)
    #[arg(long, short = 'l')]
    pub labels: Vec<String>,
}

#[derive(Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct DeleteArgs {
    /// Issue IDs to delete
    pub ids: Vec<String>,

    /// Delete reason (default: "delete")
    #[arg(long, default_value = "delete")]
    pub reason: String,

    /// Read IDs from file (one per line, # comments ignored)
    #[arg(long)]
    pub from_file: Option<PathBuf>,

    /// Delete dependents recursively
    #[arg(long)]
    pub cascade: bool,

    /// Bypass dependent checks (orphans dependents)
    #[arg(long, conflicts_with = "cascade")]
    pub force: bool,

    /// Prune tombstones from JSONL immediately
    #[arg(long)]
    pub hard: bool,

    /// Preview only, no changes
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the list command.
#[derive(Args, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ListArgs {
    /// Filter by status (can be repeated)
    #[arg(long, short = 's')]
    pub status: Vec<String>,

    /// Filter by issue type (can be repeated)
    #[arg(long = "type", short = 't')]
    pub type_: Vec<String>,

    /// Filter by assignee
    #[arg(long)]
    pub assignee: Option<String>,

    /// Filter for unassigned issues only
    #[arg(long)]
    pub unassigned: bool,

    /// Filter by specific IDs (can be repeated)
    #[arg(long)]
    pub id: Vec<String>,

    /// Filter by label (AND logic, can be repeated)
    #[arg(long, short = 'l')]
    pub label: Vec<String>,

    /// Filter by label (OR logic, can be repeated)
    #[arg(long)]
    pub label_any: Vec<String>,

    /// Filter by priority (can be repeated)
    #[arg(long, short = 'p')]
    pub priority: Vec<u8>,

    /// Filter by minimum priority (0=critical, 4=backlog)
    #[arg(long)]
    pub priority_min: Option<u8>,

    /// Filter by maximum priority
    #[arg(long)]
    pub priority_max: Option<u8>,

    /// Title contains substring
    #[arg(long)]
    pub title_contains: Option<String>,

    /// Description contains substring
    #[arg(long)]
    pub desc_contains: Option<String>,

    /// Notes contains substring
    #[arg(long)]
    pub notes_contains: Option<String>,

    /// Include closed issues (default excludes closed)
    #[arg(long, short = 'a')]
    pub all: bool,

    /// Maximum number of results (0 = unlimited, default: 50)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Sort field (`priority`, `created_at`, `updated_at`, `title`)
    #[arg(long)]
    pub sort: Option<String>,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Include deferred issues
    #[arg(long)]
    pub deferred: bool,

    /// Filter for overdue issues
    #[arg(long)]
    pub overdue: bool,

    /// Use long output format
    #[arg(long)]
    pub long: bool,

    /// Use tree/pretty output format
    #[arg(long)]
    pub pretty: bool,
}

/// Arguments for the search command.
#[derive(Args, Debug, Default)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    #[command(flatten)]
    pub filters: ListArgs,
}

#[derive(Subcommand, Debug)]
pub enum DepCommands {
    Add,
    Remove,
    List,
    Tree,
    Cycles,
}

#[derive(Subcommand, Debug)]
pub enum LabelCommands {
    Add,
    Remove,
    List,
    ListAll,
}

#[derive(Subcommand, Debug)]
pub enum CommentCommands {
    Add,
    List,
}

#[derive(Args, Debug, Clone)]
pub struct CountArgs {
    /// Group counts by field
    #[arg(long, value_enum)]
    pub by: Option<CountBy>,

    /// Filter by status (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub status: Vec<String>,

    /// Filter by issue type (repeatable or comma-separated)
    #[arg(long = "type", value_delimiter = ',')]
    pub types: Vec<String>,

    /// Filter by priority (0-4 or P0-P4; repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub priority: Vec<String>,

    /// Filter by assignee
    #[arg(long)]
    pub assignee: Option<String>,

    /// Only include unassigned issues
    #[arg(long)]
    pub unassigned: bool,

    /// Include closed and tombstone issues
    #[arg(long)]
    pub include_closed: bool,

    /// Include template issues
    #[arg(long)]
    pub include_templates: bool,

    /// Title contains substring
    #[arg(long)]
    pub title_contains: Option<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy, Eq, PartialEq)]
pub enum CountBy {
    Status,
    Priority,
    Type,
    Assignee,
    Label,
}
