mod cli;

use clap::Parser;
use colored::*;
use fuzzy_matcher::FuzzyMatcher;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;
use crate::cli::{Cli, MatchMode};

#[derive(Debug)]
struct SearchResult {
    path: PathBuf,
    score: i64,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
}

fn show_welcome_help() {
    println!("{}", "🚀 Fast File Finder".bright_cyan().bold());
    println!();
    println!("{}", "USAGE:".yellow().bold());
    println!("    {} {} {}", "ff".green().bold(), "<args>".white(), "<options>".yellow());
    println!();
    println!("{}", "EXAMPLES:".yellow().bold());
    println!("    {} {}          {}", "ff".green(), "<your_file_name>".white(), "→ Locate/s that file/s".dimmed());
    println!("    {} {}        {}", "ff".green(), "main.rs".white(), "→ Find main.rs files".dimmed());
    println!();
    println!("{}", "OPTIONS:".yellow().bold());
    println!("    {} {}        {}", "--path".blue(), "<dir>".white(), "Search in specific directory".dimmed());
    println!("    {} {}           {}", "--copy".blue(), "     ".white(), "Copy path to clipboard".dimmed());
    println!("    {} {}       {}", "--hidden".blue(), "   ".white(), "Include hidden files".dimmed());
    println!("    {} {}    {}", "--dirs-only".blue(), " ".white(), "Find only directories".dimmed());
    println!("    {} {}   {}", "--files-only".blue(), "".white(), "Find only files".dimmed());
    println!();
    println!("   Type {} for detailed help", "ff --help".green().bold());
    println!("{} Press {} to cancel search anytime", "⚠️".bright_yellow(), "Ctrl+C".red().bold());
}

fn search_files(
    search_path: &Path,
    pattern: &str,
    include_hidden: bool,
    dirs_only: bool,
    files_only: bool,
    limit: usize,
    show_details: bool,
    match_mode: &MatchMode,
) -> Vec<SearchResult> {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let mut results = Vec::new();
    let mut files_scanned = 0;
    let mut dirs_scanned = 0;
    let mut last_update = std::time::Instant::now();

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\n Search cancelled by user");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    println!("{} Searching in: {}", "🔍".yellow(), search_path.display().to_string().cyan());
    println!("{} Match mode: {} | Press {} to cancel",
             "💡".dimmed(),
             format!("{:?}", match_mode).blue(),
             "Ctrl+C".red()
    );

    let walker = WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Check if search was cancelled
            if !running.load(Ordering::SeqCst) {
                return false;
            }

            if !include_hidden {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') && name.len() > 1 {
                        return false;
                    }
                }
            }
            true
        });

    for entry in walker {
        // Check cancellation frequently
        if !running.load(Ordering::SeqCst) {
            println!("{} Search stopped", "⏹️".red());
            break;
        }

        match entry {
            Ok(entry) => {
                let path = entry.path();
                let is_dir = path.is_dir();

                // Count files and directories
                if is_dir {
                    dirs_scanned += 1;
                } else {
                    files_scanned += 1;
                }

                // Apply type filters early
                if dirs_only && !is_dir {
                    continue;
                }
                if files_only && is_dir {
                    continue;
                }

                // Show progress every second
                if last_update.elapsed().as_secs() >= 1 {
                    eprint!("\r{} Scanned {} files, {} dirs... {}",
                            "📁".yellow(),
                            files_scanned,
                            dirs_scanned,
                            "(Ctrl+C to cancel)".dimmed()
                    );
                    io::stdout().flush().unwrap();
                    last_update = std::time::Instant::now();
                }

                // Extract filename and perform matching
                if let Some(file_name) = path.file_name() {
                    if let Some(name_str) = file_name.to_str() {
                        let score = match match_mode {
                            MatchMode::Fuzzy => {
                                matcher.fuzzy_match(name_str, pattern)
                            }
                            MatchMode::Exact => {
                                if name_str.to_lowercase().contains(&pattern.to_lowercase()) {
                                    Some(100) // High score for exact matches
                                } else {
                                    None
                                }
                            }
                        };

                        // Only proceed if we have a match
                        if let Some(score) = score {
                            let (size, modified) = if show_details {
                                get_file_metadata(&entry)
                            } else {
                                (None, None)
                            };

                            results.push(SearchResult {
                                path: path.to_path_buf(),
                                score,
                                is_dir,
                                size,
                                modified,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                // Skip permission errors silently, warn about others
                if !e.to_string().contains("Permission denied") {
                    eprintln!("{} {}", "⚠️".yellow(), format!("Warning: {}", e).dimmed());
                }
            }
        }
    }


    // Clear progress line and show final count
    if last_update.elapsed().as_millis() > 100 { // If we showed any progress
        eprint!("\r{}", " ".repeat(70)); // Clear line
        eprint!("\r");
        if files_scanned > 0 || dirs_scanned > 0 {
            println!("{} Scanned {} files and {} directories total",
                     "📊".green(), files_scanned, dirs_scanned);
        }
    }

    // Only sort and return results if search completed
    if running.load(Ordering::SeqCst) {
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
    }

    results
}

fn display_results(results: &[SearchResult], show_details: bool) {
    if results.is_empty() {
        println!();
        println!("{}", "No files found matching the pattern".bright_red());
        println!();
        return;
    }

    println!();
    println!("{} Found {} match(es):", "✅".green(), results.len().to_string().bright_green().bold());
    println!();

    for (index, result) in results.iter().enumerate() {
        let index_str = format!("{:2}", index + 1);
        let type_icon = if result.is_dir { "📁" } else { "📄" };
        let path_str = result.path.display().to_string();

        let mut line = format!(
            "{} {} {}",
            index_str.bright_blue().bold(),
            type_icon,
            path_str.white(),
        );

        if show_details {
            if let Some(size) = result.size {
                line.push_str(&format!(" {}", format_size(size).dimmed()));
            }
            if let Some(modified) = result.modified {
                if let Ok(elapsed) = modified.elapsed() {
                    line.push_str(&format!(" {}", format_time_ago(elapsed).dimmed()));
                }
            }
        }

        line.push_str(&format!(" {}", format!("({})", result.score).bright_black()));
        println!("{}", line);
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{:.0}{}", size, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

fn format_time_ago(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();

    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

fn get_file_metadata(entry: &walkdir::DirEntry) -> (Option<u64>, Option<std::time::SystemTime>) {
    match entry.metadata() {
        Ok(meta) => (
            if meta.is_file() {
                Some(meta.len())
            } else {
                None
            },
            meta.modified().ok()
        ),
        Err(_) => (None, None)
    }
}


fn interactive_select(results: &[SearchResult]) -> Option<&SearchResult> {
    if results.is_empty() {
        return None;
    }

    if results.len() == 1 {
        println!();
        println!("Auto-selecting the only match...");
        return Some(&results[0]);
    }

    println!();
    loop {
        print!(
            "{} Enter number ({}-{}) or '{}' to quit: ",
            "❓".cyan(),
            "1".bright_green(),
            results.len().to_string().bright_green(),
            "q".bright_red()
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            continue;
        }

        let input = input.trim().to_lowercase();

        if input == "q" || input == "quit" || input == "exit" {
            println!("Selection cancelled");
            return None;
        }

        if let Ok(num) = input.parse::<usize>() {
            if num >= 1 && num <= results.len() {
                return Some(&results[num - 1]);
            }
        }

        println!("{} Invalid selection. Please enter a number between 1-{} or 'q' to quit.",
                 "❌".red(), results.len());
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    println!();
    println!("{} Path copied to clipboard:", "📋".green());
    println!("   {}", text.cyan());
    Ok(())
}

fn change_directory(path: &Path) {
    let dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    println!();
    println!("{} Opening new terminal in: {}", "🚀".green(), dir.display().to_string().cyan());

    if let Err(e) = spawn_terminal(dir) {
        eprintln!("{} Failed to open terminal: {}", "❌".red(), e);
        println!("{} Fallback - copy this command:", "💡".yellow());

        #[cfg(target_os = "windows")]
        println!("cd /d {}", dir.display());

        #[cfg(not(target_os = "windows"))]
        println!("cd \"{}\"", dir.display());
    }
}

fn spawn_terminal(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        let path_str = path.display().to_string();

        // This should work more reliably
        Command::new("cmd")
            .args(&[
                "/C",
                "start",
                "/D",
                &path_str,  // Set directory with /D flag
                "cmd"
            ])
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try different terminals in order of preference
        let terminals = [
            ("gnome-terminal", vec!["--working-directory", &path.display().to_string()]),
            ("konsole", vec!["--workdir", &path.display().to_string()]),
            ("xfce4-terminal", vec!["--working-directory", &path.display().to_string()]),
            ("alacritty", vec!["--working-directory", &path.display().to_string()]),
            ("kitty", vec!["--directory", &path.display().to_string()]),
            ("x-terminal-emulator", vec!["--working-directory", &path.display().to_string()]),
        ];

        let mut success = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal).args(args).spawn().is_ok() {
                success = true;
                break;
            }
        }

        if !success {
            // Fallback: try to open any terminal and cd manually
            let fallback_cmd = format!("cd '{}' && exec $SHELL", path.display());
            Command::new("x-terminal-emulator")
                .args(&["-e", "bash", "-c", &fallback_cmd])
                .spawn()
                .or_else(|_| {
                    Command::new("xterm")
                        .args(&["-e", "bash", "-c", &fallback_cmd])
                        .spawn()
                })?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: Use AppleScript to open Terminal.app
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && clear\"",
            path.display()
        );
        Command::new("osascript")
            .args(&["-e", &script])
            .spawn()?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let pattern = match cli.pattern {
        Some(p) => p,
        None => {
            show_welcome_help();
            return Ok(());
        }
    };

    if pattern.trim().is_empty() {
        println!("{} Search pattern cannot be empty", "❌".red());
        println!("{} Example: {}", "💡".yellow(), "ff config.json".green());
        return Ok(());
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
    println!("   Pattern: {}", pattern.bright_white().bold());
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
    let results = search_files(
        &search_path,
        &pattern,
        cli.hidden,
        cli.dirs_only,
        cli.files_only,
        cli.limit,
        cli.details,
        &cli.match_mode
    );
    let search_duration = start_time.elapsed();

    // Display results
    display_results(&results, cli.details);

    if !results.is_empty() {
        println!();
        println!(
            "{} Search completed in {:.1}ms",
            "⚡".yellow(),
            search_duration.as_millis()
        );

        // Only do interactive selection if an action is requested
        if cli.copy || cli.terminal {
            if let Some(selected) = interactive_select(&results) {
                if cli.copy {
                    copy_to_clipboard(&selected.path.display().to_string())?;
                } else if cli.terminal {
                    change_directory(&selected.path);
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