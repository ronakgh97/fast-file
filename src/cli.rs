use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ff",
    version = "0.0.1-beta.1",
    about = "Fast File - Locate your filesystem (BLAZINGLY FAST🔥)",
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
    ff main --path /codes       Search for 'main' in /codes directory
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

    /// [Search] Include hidden files and directories (.git, .env, etc.)
    #[arg(short = 'h', long)]
    pub hidden: bool,

    /// [Search] Only match files (exclude directories)
    #[arg(short = 'f', long)]
    pub files_only: bool,

    /// [Search] Only match directories
    #[arg(short = 'd', long)]
    pub dirs_only: bool,

    /// [Search] Matching mode: fuzzy or exact
    #[arg(short = 'm', long, value_enum, default_value = "fuzzy")]
    pub match_mode: MatchMode,

    /// [Output] Maximum number of results to show
    #[arg(short = 'l', long, default_value = "10", value_name = "NUM")]
    pub limit: usize,

    /// [Output] Copy selected path to clipboard instead of navigating
    #[arg(short = 'c', long)]
    pub copy: bool,

    /// [Output] Show detailed information (file sizes, dates)
    #[arg(long)]
    pub details: bool,

    /// [Output] Open new terminal window
    #[arg(short = 't', long)]
    pub terminal: bool,

    /// [Performance] Use parallel processing with Rayon
    #[arg(long = "pl")]
    pub parallel: bool,

    /// [Performance] Number of threads to use (default: auto-detect)
    #[arg(long = "th", value_name = "THREADS")]
    pub threads: Option<usize>,

    /// [Performance] Use maximum CPU cores (CPU_COUNT * 2)
    #[arg(long = "mx")]
    pub max_cpu: bool,
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