mod cli;
mod util;
mod search;
mod config;

use clap::Parser;
use colored::*;
use std::path::{PathBuf};
use crate::cli::{Cli};
use figlet_rs::FIGfont;
use config::Config;

#[derive(Debug)]
struct SearchResult {
    path: PathBuf,
    score: i64,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
    pub content_matches: Vec<ContentMatch>,
    pub search_type: SearchType,
}

#[derive(Debug, Clone)]
pub struct ContentMatch {
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone)]
pub enum SearchType {
    FileName,
    Content,
    Hybrid, // Both filename and content
}

fn show_banner() {
    let font = FIGfont::standard().unwrap();
    let banner = font.convert("ff-fast file").unwrap();

    let text = banner.to_string();
    let lines: Vec<&str> = text.lines().collect();

    // Gradient palette
    let gradient = vec![
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::BrightRed,
        Color::BrightGreen,
        Color::BrightBlue,
    ];

    // Print each line with gradient color
    for (i, line) in lines.iter().enumerate() {
        let color = gradient[i % gradient.len()];
        println!("{}", line.color(color).bold());
    }
}

fn show_welcome_help() {

    show_banner();

    println!("\n{}", "Fast File Finder".bright_cyan().bold());

    println!("\n{}", "USAGE:".yellow().bold());
    println!("  {} {} {}", "ff".green().bold(), "<args>".white(), "<options>".yellow());

    println!("\n{}", "EXAMPLES:".yellow().bold());
    println!("  {} {}      {}", "ff".green(), "<your_file_name>".white(), "→ Locate file(s)".dimmed());
    println!("  {} {}          {}", "ff".green(), "main.rs".white(), "→ Find main.rs files".dimmed());

    println!("\n{}", "OPTIONS:".yellow().bold());
    println!("  {:<12} {:<10} {}", "--path".blue(), "<dir>".white(), "Search in directory".dimmed());
    println!("  {:<12} {:<10} {}", "--copy".blue(), "" , "Copy path to clipboard".dimmed());
    println!("  {:<12} {:<10} {}", "--hidden".blue(), "" , "Include hidden files".dimmed());
    println!("  {:<12} {:<10} {}", "--dirs-only".blue(), "" , "Find only directories".dimmed());
    println!("  {:<12} {:<10} {}", "--files-only".blue(), "" , "Find only files".dimmed());

    println!("\n  Type {} for detailed help", "ff --help".green().bold());
    println!("  {} Press {} anytime to cancel", "⚠️".bright_yellow(), "Ctrl+C".red().bold());
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_with_safeguard();
    let cli = Cli::parse();

    //Calculate effective values (CLI overrides config)
    let effective_hidden = cli.hidden || config.include_hidden;
    let effective_details = cli.details || config.output_options.show_details;
    let optimal_threads = config.get_effective_thread_count(cli.threads, cli.max_cpu);

    let filename_pattern = cli.pattern.clone();
    let content_pattern = cli.content.clone();

    // Validate that we have at least one search pattern
    if filename_pattern.is_none() && content_pattern.is_none() {
        show_welcome_help();
        return Ok(());
    }

    // Convert to Option<&str> for function calls
    let filename_pattern = filename_pattern.as_deref();
    let content_pattern = content_pattern.as_deref();

    if cli.parallel {
        rayon::ThreadPoolBuilder::new()
            .num_threads(optimal_threads)
            .thread_name(|i| format!("ff-{}", i))
            .build_global()
            .unwrap_or_else(|_| {});
    }

    // NEW - handles both filename and content patterns:
    match (&filename_pattern, &content_pattern) {
        (Some(fp), _) if fp.trim().is_empty() => {
            println!("{} Search pattern cannot be empty", "❌".red());
            println!("{} Example: {}", "💡".yellow(), "ff config.json".green());
            return Ok(());
        }
        (None, Some(cp)) if cp.trim().is_empty() => {
            println!("{} Content pattern cannot be empty", "❌".red());
            println!("{} Example: {}", "💡".yellow(), "ff --content \"hello world\"".green());
            return Ok(());
        }
        (None, None) => {
            show_welcome_help();
            return Ok(());
        }
        _ => {} // Continue with search
    }
    
    let search_path = cli.path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    if !search_path.exists() {
        println!("{} Search path does not exist: {}", "❌".red(), search_path.display().to_string().red());
        println!("{} Current directory: {}", "📍".yellow(), std::env::current_dir().unwrap().display().to_string().cyan());
        return Ok(());
    }

    // Show search summary
    println!("{}", "🔎 SEARCH SUMMARY".yellow().bold());
    if let Some(ref pattern) = cli.pattern {
        println!(" Filename pattern: {}", pattern.bright_white().bold());
    }
    if let Some(ref pattern) = cli.content {
        println!(" Content pattern: {}", pattern.bright_white().bold());
    }
    println!("   Path: {}", search_path.display().to_string().cyan());
    if cli.dirs_only {
        println!("   Filter: {} only", "directories".blue());
    } else if cli.files_only {
        println!("   Filter: {} only", "files".blue());
    }
    if cli.hidden {
        println!("   Including: {} files", "hidden".blue());
    }
    println!();

    // Perform search with cancellation support
    let start_time = std::time::Instant::now();
    let results = if cli.parallel {
        search::search_files_parallel(
            &search_path,
            filename_pattern,
            content_pattern,
            effective_hidden,
            cli.dirs_only,
            cli.files_only,
            cli.limit,
            effective_details,
            &cli.match_mode,
            optimal_threads,
            &config,
        )
    } else {
        search::search_files(
            &search_path,
            filename_pattern,
            content_pattern,
            effective_hidden,
            cli.dirs_only,
            cli.files_only,
            cli.limit,
            effective_details,
            &cli.match_mode,
            &config,
        )
    };

    let search_duration = start_time.elapsed();

    // Display results
    util::display_results(&results, cli.details);

    if !results.is_empty() {
        println!();
        println!(
            "{} Search completed in {:.1}ms",
            "⚡".yellow(),
            search_duration.as_millis()
        );

        // Only do interactive selection if an action is requested
        if cli.copy || cli.terminal {
            if let Some(selected) = util::interactive_select(&results) {
                if cli.copy {
                    util::copy_to_clipboard(&selected.path.display().to_string())?;
                } else if cli.terminal {
                    util::change_directory(&selected.path);
                }
            }
        } else {
            // Default behavior: just show available actions
            println!();
            println!("{} Found {} files. Use these flags for actions:",
                     "💡".yellow(),
                     results.len().to_string().green()
            );
            println!("   {} - Open selected file's directory in new terminal", "--t".blue());
            println!("   {} - Copy selected file's path to clipboard", "--c".blue());
        }
    }

    Ok(())
}