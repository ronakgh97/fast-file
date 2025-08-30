use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ff",
    version = "0.0.1-beta.1",
    about = "Fast File Finder - Locate your filesystem (BLAZINGLY FAST🔥)",
    long_about = None,
    help_template = "\
{name} {version}
{about}

USAGE:
    {usage}

OPTIONS:
{options}

{subcommands}{after-help}
",
    after_help = "\
EXAMPLES:
    ff config.json              Search for config.json in current directory
    ff main --path /codes       Search for 'main' in /codes directory
    ff docker --dirs-only       Find only directories matching 'docker'
    ff package --copy           Copy the selected file path to clipboard
    ff \"*.rs\" --files-only     Find only Rust files (use quotes for wildcards)
"
)]
pub struct Cli {
    /// File or directory pattern to search for
    pub pattern: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// [Navigation] Directory to search in (default: current directory)
    #[arg(short, long, value_name = "PATH")]
    pub path: Option<String>,

    /// [Output] Copy selected path to clipboard instead of navigating
    #[arg(short, long)]
    pub copy: bool,

    /// [Search] Include hidden files and directories (.git, .env, etc.)
    #[arg(short, long)]
    pub hidden: bool,

    /// [Output] Maximum number of results to show
    #[arg(short = 'l', long, default_value = "10", value_name = "NUM")]
    pub limit: usize,

    /// [Search] Only match directories
    #[arg(short = 'd', long)]
    pub dirs_only: bool,

    /// [Search] Only match files (exclude directories)
    #[arg(short = 'f', long)]
    pub files_only: bool,

    /// [Output] Show detailed information (file sizes, dates)
    #[arg(long)]
    pub details: bool,

    /// [Navigation] Open new terminal window
    #[arg(short = 't', long)]
    pub terminal: bool,

    /// [Search] Matching mode: fuzzy or exact
    #[arg(short = 'm', long, value_enum, default_value = "fuzzy")]
    pub match_mode: MatchMode,
}

#[derive(Subcommand)]
pub enum Commands {
    // Reserve space for future subcommands
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MatchMode {
    /// Fuzzy matching (default) - finds partial matches
    Fuzzy,
    /// Exact matching - only exact substring matches
    Exact,
}


