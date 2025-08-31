mod cli;

use clap::Parser;
use colored::*;
use rayon::prelude::*;
use fuzzy_matcher::FuzzyMatcher;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;
use crate::cli::{Cli, MatchMode};
use std::thread;
use std::time::{Duration, Instant};
use figlet_rs::FIGfont;

#[derive(Debug)]
struct SearchResult {
    path: PathBuf,
    score: i64,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
}

fn show_banner() {
    let font = FIGfont::standard().unwrap();
    let banner = font.convert("ff-fast file").unwrap();

    let text = banner.to_string();
    let lines: Vec<&str> = text.lines().collect();

    // Gradient palette
    let gradient = vec![
        Color::Red,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
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
    println!("   Match mode: {} | Press {} to cancel", format!("{:?}", match_mode).blue(),"Ctrl+C".red());

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
            println!("{} Search stopped", "🛑".red());
            break;
        }

        match entry {
            Ok(entry) => {
                let path = entry.path();
                //eprintln!("❕Scanning: -> {}  ", path.display().to_string().green());
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
                        let score = get_best_match_score(name_str, pattern, &matcher, match_mode);

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

fn search_files_parallel(
    search_path: &Path,
    pattern: &str,
    include_hidden: bool,
    dirs_only: bool,
    files_only: bool,
    limit: usize,
    show_details: bool,
    match_mode: &MatchMode,
    threads: usize,
) -> Vec<SearchResult> {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let cpu_cores = num_cpus::get();

    println!("{} Searching in: {} {}",
             "🔍".yellow(),
             search_path.display().to_string().cyan(),
             format!("(Parallel Mode - {} cores)", cpu_cores).green()
    );
    println!("   Using {} threads on {} CPU cores", threads, cpu_cores);
    println!("   Match mode: {} | Press Ctrl+C to cancel", format!("{:?}", match_mode).blue());

    // Add Ctrl+C handling for parallel mode
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\n Search cancelled by user (parallel mode)");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // Collect all paths first
    let all_paths: Vec<PathBuf> = WalkDir::new(search_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if !include_hidden {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') && name.len() > 1 {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|entry| entry.ok().map(|e| e.path().to_path_buf()))
        .collect();

    println!("🚀 Processing {} paths using {} CPU cores",
             all_paths.len(), cpu_cores);

    let total_paths = all_paths.len();

    // Atomic counters for progress tracking
    let files_processed = Arc::new(AtomicUsize::new(0));
    let dirs_processed = Arc::new(AtomicUsize::new(0));
    let files_scanned = Arc::new(AtomicUsize::new(0));
    let dirs_scanned = Arc::new(AtomicUsize::new(0));
    let processing_complete = Arc::new(AtomicBool::new(false));

    // Progress display thread with cancellation check
    let files_p = files_processed.clone();
    let dirs_p = dirs_processed.clone();
    let files_s = files_scanned.clone();
    let dirs_s = dirs_scanned.clone();
    let complete_flag = processing_complete.clone();
    let running_progress = running.clone();

    let progress_thread = thread::spawn(move || {
        let mut last_update = Instant::now();

        while !complete_flag.load(Ordering::Relaxed) && running_progress.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));

            if last_update.elapsed() >= Duration::from_secs(1) {
                let processed = files_p.load(Ordering::Relaxed) + dirs_p.load(Ordering::Relaxed);
                let scanned_f = files_s.load(Ordering::Relaxed);
                let scanned_d = dirs_s.load(Ordering::Relaxed);

                eprint!("\r{} Processed {}/{} paths, {} files, {} dirs... {}",
                        "📁".yellow(),
                        processed,
                        total_paths,
                        scanned_f,
                        scanned_d,
                        "(Parallel)".green()
                );
                io::stdout().flush().unwrap();
                last_update = Instant::now();
            }
        }

        // Clear progress line
        eprint!("\r{}", " ".repeat(80));
        eprint!("\r");

        // Show appropriate completion message
        if running_progress.load(Ordering::Relaxed) {
            let final_files = files_s.load(Ordering::Relaxed);
            let final_dirs = dirs_s.load(Ordering::Relaxed);
            println!("{} Scanned {} files and {} directories total (parallel processing complete)",
                     "📊".green(), final_files, final_dirs);
        } else {
            println!("{} Parallel search stopped", "🛑".red());
        }
    });

    // Parallel processing with cancellation checks
    let mut results: Vec<SearchResult> = all_paths
        .into_par_iter()
        .filter_map(|path| {
            // Check for cancellation in parallel tasks
            if !running.load(Ordering::Relaxed) {
                return None;
            }

            let is_dir = path.is_dir();

            // Update processing counters
            if is_dir {
                dirs_processed.fetch_add(1, Ordering::Relaxed);
            } else {
                files_processed.fetch_add(1, Ordering::Relaxed);
            }

            // Apply type filters
            if dirs_only && !is_dir { return None; }
            if files_only && is_dir { return None; }

            let file_name = path.file_name()?.to_str()?;
            let score = get_best_match_score(file_name, pattern, &matcher, match_mode)?;

            // Count matched files/dirs
            if is_dir {
                dirs_scanned.fetch_add(1, Ordering::Relaxed);
            } else {
                files_scanned.fetch_add(1, Ordering::Relaxed);
            }

            let (size, modified) = if show_details {
                if let Ok(metadata) = path.metadata() {
                    (
                        if metadata.is_file() { Some(metadata.len()) } else { None },
                        metadata.modified().ok()
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            Some(SearchResult { path, score, is_dir, size, modified })
        })
        .collect();

    // Signal completion and wait for progress thread
    processing_complete.store(true, Ordering::Relaxed);
    progress_thread.join().unwrap();

    // Only sort and return results if search wasn't cancelled
    if running.load(Ordering::Relaxed) {
        results.par_sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
    } else {
        // Return partial results if cancelled
        results.par_sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit.min(results.len()));
    }

    results
}

fn get_best_match_score(
    filename: &str,
    pattern: &str,
    matcher: &fuzzy_matcher::skim::SkimMatcherV2,
    match_mode: &MatchMode
) -> Option<i64> {
    match match_mode {
        MatchMode::Fuzzy => {
            // Multi-algorithm fusion for fuzzy mode
            let fuzzy_score = matcher.fuzzy_match(filename, pattern);
            let exact_score = if filename.to_lowercase().contains(&pattern.to_lowercase()) {
                Some(100)
            } else {
                None
            };
            let prefix_score = if filename.to_lowercase().starts_with(&pattern.to_lowercase()) {
                Some(150)
            } else {
                None
            };

            // Return the best score
            [fuzzy_score, exact_score, prefix_score]
                .into_iter()
                .flatten()
                .max()
        }

        MatchMode::Exact => {
            // Keep exact mode simple
            if filename.to_lowercase().contains(&pattern.to_lowercase()) {
                Some(100)
            } else {
                None
            }
        }
    }
}

fn get_optimal_threads(cli: &Cli) -> usize {
    if let Some(threads) = cli.threads {
        // User specified exact thread count
        threads
    } else if cli.max_cpu {
        // Maximum aggressive mode
        num_cpus::get() * 2
    } else {
        // Default: one thread per core (raspberry pi)
        num_cpus::get()
    }
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
        let type_icon = get_file_icon(&result);
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

fn get_file_icon(result: &SearchResult) -> &'static str {
    if result.is_dir {
        return "📁";
    }

    match result.path.extension().and_then(|s| s.to_str()) {

        // Programming / Scripting
        Some("rs")               => "🦀",
        Some("js") | Some("ts")  => "📜",
        Some("py")               => "🐍",
        Some("java")             => "☕",
        Some("cpp") | Some("cxx")| Some("cc") => "💠",
        Some("c")                => "🔵",
        Some("h") | Some("hpp")  => "📘",
        Some("go")               => "🐹",
        Some("rb")               => "💎",
        Some("php")              => "🐘",
        Some("sh") | Some("bash")=> "🐚",
        Some("swift")            => "🍎",
        Some("kt") | Some("kts") => "🤖",
        Some("cs")               => "🎯",

        // Data / Config
        Some("json")             => "📋",
        Some("yaml") | Some("yml") => "⚙️",
        Some("toml")             => "🛠️",
        Some("ini")              => "📑",
        Some("csv")              => "📊",
        Some("xml")              => "🗂️",

        // Markup / Docs
        Some("md")               => "📝",
        Some("txt")              => "📄",
        Some("html") | Some("htm") => "🌐",
        Some("css")              => "🎨",
        Some("pdf")              => "📕",

        // Images
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("bmp") | Some("svg") => "🖼️",
        Some("ico")              => "🔖",

        // Video
        Some("mp4") | Some("mkv") | Some("avi") | Some("mov") | Some("webm") => "🎬",

        // Audio
        Some("mp3") | Some("wav") | Some("flac") | Some("ogg") | Some("m4a") => "🎵",

        // Archives
        Some("zip") | Some("tar") | Some("gz") | Some("bz2") | Some("xz") | Some("7z") => "📦",

        // Misc
        Some("exe") | Some("bin") | Some("dll") => "⚙️",
        Some("lock")             => "🔒",
        Some("log")              => "📜",

        _ => "📄", // Default file
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
        let path_str = path.display().to_string(); // Create string once

        let terminals = [
            ("gnome-terminal", vec!["--working-directory", &path_str]),
            ("konsole", vec!["--workdir", &path_str]),
            ("xfce4-terminal", vec!["--working-directory", &path_str]),
            ("alacritty", vec!["--working-directory", &path_str]),
            ("kitty", vec!["--directory", &path_str]),
            ("wezterm", vec!["start", "--cwd", &path_str]),
        ];

        let mut success = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal).args(args).spawn().is_ok() {
                success = true;
                break;
            }
        }

        if !success {
            return Err("No compatible terminal found on Linux".into());
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: Use AppleScript to open Terminal.app
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && clear\" in window 1",
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

    let optimal_threads = get_optimal_threads(&cli);

    if cli.parallel {
        rayon::ThreadPoolBuilder::new()
            .num_threads(optimal_threads)
            .thread_name(|i| format!("ff-{}", i))
            .build_global()
            .unwrap_or_else(|_| {});
    }

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
    let results = if cli.parallel {
        search_files_parallel(
            &search_path,
            &pattern,
            cli.hidden,
            cli.dirs_only,
            cli.files_only,
            cli.limit,
            cli.details,
            &cli.match_mode,
            optimal_threads
        )
    } else {
        search_files(
            &search_path,
            &pattern,
            cli.hidden,
            cli.dirs_only,
            cli.files_only,
            cli.limit,
            cli.details,
            &cli.match_mode
        )
    };

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