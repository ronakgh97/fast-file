use rayon::iter::ParallelIterator;
use std::{io, thread};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use colored::Colorize;
use fuzzy_matcher::FuzzyMatcher;
use rayon::iter::IntoParallelIterator;
use rayon::prelude::ParallelSliceMut;
use walkdir::WalkDir;
use crate::cli::MatchMode;
use crate::{util, SearchResult};
use std::fs::File;
use std::io::{BufRead, BufReader};
use crate::{ContentMatch, SearchType};

pub fn search_file_content(
    file_path: &Path,
    pattern: &str,
    match_mode: &MatchMode,
) -> Result<Vec<ContentMatch>, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut matches = Vec::new();

    let pattern_lower = pattern.to_lowercase();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let line_lower = line.to_lowercase();

        let found = match match_mode {
            MatchMode::Exact => line_lower.contains(&pattern_lower),
            MatchMode::Fuzzy => {
                // Simple fuzzy: exact match OR word boundary match
                line_lower.contains(&pattern_lower) ||
                    fuzzy_matcher::skim::SkimMatcherV2::default()
                        .fuzzy_match(&line, pattern).is_some()
            }
        };

        if found {
            // Find all occurrences in this line
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&pattern_lower) {
                let actual_pos = start + pos;
                matches.push(ContentMatch {
                    line_number: line_num + 1,
                    line_content: line.clone(),
                    match_start: actual_pos,
                    match_end: actual_pos + pattern.len(),
                });
                start = actual_pos + 1;
            }
        }
    }

    Ok(matches)
}

pub fn search_files(
    search_path: &Path,
    filename_pattern: Option<&str>,
    content_pattern: Option<&str>,
    include_hidden: bool,
    dirs_only: bool,
    files_only: bool,
    limit: usize,
    show_details: bool,
    match_mode: &MatchMode,
    config: &crate::config::Config,
) -> Vec<SearchResult> {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let mut results = Vec::new();
    let mut files_scanned = 0;
    let mut dirs_scanned = 0;
    let mut last_update = std::time::Instant::now();

    // Determine search type
    let search_type = match (filename_pattern, content_pattern) {
        (Some(_), Some(_)) => SearchType::Hybrid,
        (Some(_), None) => SearchType::FileName,
        (None, Some(_)) => SearchType::Content,
        (None, None) => return results, // No search pattern
    };

    // Set up Ctrl+C handler (your existing code)
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\n🛑 Search cancelled by user");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    println!("{} Searching in: {}", "🔍".yellow(), search_path.display().to_string().cyan());
    println!(" Search type: {} | Press {} to cancel",
             format!("{:?}", search_type).blue(), "Ctrl+C".red());

    let walker = WalkDir::new(search_path)
        .follow_links(config.follow_symlinks)
        .into_iter()
        .filter_entry(|e| {
            if !running.load(Ordering::SeqCst) {
                return false;
            }
            let effective_hidden = include_hidden || config.include_hidden;
            if !effective_hidden {  // Check both CLI and config
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') && name.len() > 1 {
                        return false;
                    }
                }
            }

            //Use config ignore directories
            if let Some(name) = e.file_name().to_str() {
                if config.should_ignore_directory(name) {
                    return false;
                }

                // 🆕 Use config ignore file patterns
                if config.should_ignore_file(name) {
                    return false;
                }
            }

            if let Ok(metadata) = e.metadata() {
                if metadata.is_file() && metadata.len() > config.max_file_size_mb * 1024 * 1024 {
                    return false;
                }
            }

            true
        });

    for entry in walker {
        if !running.load(Ordering::SeqCst) {
            println!("{} Search stopped", "🛑".red());
            break;
        }

        match entry {
            Ok(entry) => {
                let path = entry.path();
                let is_dir = path.is_dir();

                // Count and apply filters (your existing code)
                if is_dir {
                    dirs_scanned += 1;
                } else {
                    files_scanned += 1;
                }

                if dirs_only && !is_dir { continue; }
                if files_only && is_dir { continue; }

                // Progress update (existing code)
                if last_update.elapsed().as_secs() >= 1 {
                    eprint!("\r{} Scanned {} files, {} dirs... {}",
                            "📁".yellow(), files_scanned, dirs_scanned,
                            "(Ctrl+C to cancel)".dimmed()
                    );
                    io::stdout().flush().unwrap();
                    last_update = std::time::Instant::now();
                }

                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    let mut filename_score = None;
                    let mut content_matches = Vec::new();

                    // Check filename match
                    if let Some(pattern) = filename_pattern {
                        filename_score = get_best_match_score(file_name, pattern, &matcher, match_mode);
                    }

                    // Check content match (only for files, not directories)
                    if let Some(pattern) = content_pattern {
                        if !is_dir && config.is_content_searchable(&path) {
                            if let Ok(matches) = search_file_content(path, pattern, match_mode) {
                                if !matches.is_empty() {
                                    content_matches = matches;
                                }
                            }
                        }
                    }

                    // Determine if this is a match and calculate score
                    let (is_match, final_score) = match search_type {
                        SearchType::FileName => (filename_score.is_some(), filename_score.unwrap_or(0)),
                        SearchType::Content => (!content_matches.is_empty(), if !content_matches.is_empty() { 100 } else { 0 }),
                        SearchType::Hybrid => {
                            let has_filename = filename_score.is_some();
                            let has_content = !content_matches.is_empty();
                            let score = filename_score.unwrap_or(0) + if has_content { 50 } else { 0 };
                            (has_filename || has_content, score)
                        }
                    };

                    if is_match {
                        let (size, modified) = if show_details {
                            util::get_file_metadata(&entry)
                        } else {
                            (None, None)
                        };

                        results.push(SearchResult {
                            path: path.to_path_buf(),
                            score: final_score,
                            is_dir,
                            size,
                            modified,
                            content_matches,
                            search_type: search_type.clone(),
                        });
                    }
                }
            }
            Err(e) => {
                if !e.to_string().contains("Permission denied") {
                    eprintln!("{} {}", "⚠️".yellow(), format!("Warning: {}", e).dimmed());
                }
            }
        }
    }

    // Clean up and sort results (your existing code)
    if last_update.elapsed().as_millis() > 100 {
        eprint!("\r{}", " ".repeat(70));
        eprint!("\r");
    }
    if files_scanned > 0 || dirs_scanned > 0 {
        println!("{} Scanned {} files and {} directories total",
                 "📊".green(), files_scanned, dirs_scanned);
    }

    if running.load(Ordering::SeqCst) {
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
    }

    results
}


pub fn search_files_parallel(
    search_path: &Path,
    filename_pattern: Option<&str>,
    content_pattern: Option<&str>,
    include_hidden: bool,
    dirs_only: bool,
    files_only: bool,
    limit: usize,
    show_details: bool,
    match_mode: &MatchMode,
    threads: usize,
    config: &crate::config::Config,
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

    // Determine and display search type
    let search_type = match (filename_pattern, content_pattern) {
        (Some(_), Some(_)) => SearchType::Hybrid,
        (Some(_), None) => SearchType::FileName,
        (None, Some(_)) => SearchType::Content,
        (None, None) => SearchType::FileName, // fallback
    };
    println!("   Search type: {}", format!("{:?}", search_type).blue());

    // Add Ctrl+C handling for parallel mode
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\n🛑 Search cancelled by user (parallel mode)");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // Collect all paths first
    let all_paths: Vec<PathBuf> = WalkDir::new(search_path)
        .follow_links(config.follow_symlinks)
        .into_iter()
        .filter_entry(|e| {
            let effective_hidden = include_hidden || config.include_hidden;
            if !effective_hidden {  //Check both CLI and config
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') && name.len() > 1 {
                        return false;
                    }
                }
            }
            if let Some(name) = e.file_name().to_str() {
                if config.should_ignore_directory(name) {
                    return false;
                }

                //  Use config ignore file patterns
                if config.should_ignore_file(name) {
                    return false;
                }
            }

            true

        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            // Skip large files based on config
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() && metadata.len() > config.max_file_size_mb * 1024 * 1024 {
                    return false;
                }
            }
            true
        })
        .map(|entry| entry.path().to_path_buf())
        .take(config.max_files_per_search)  //  Use config limit
        .collect();

    println!("🚀 Processing {} paths using {} CPU cores",
             all_paths.len(), cpu_cores);

    let total_paths = all_paths.len();

    if total_paths >= config.max_files_per_search {
        println!("⚠️  Limited to {} files per config setting", config.max_files_per_search);
    }

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

    // **NEW: Enhanced parallel processing with content search support**
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

            // **NEW: Content and filename matching logic**
            let mut content_matches = Vec::new();
            let mut filename_score = None;

            // Check filename match
            if let Some(pattern) = filename_pattern {
                filename_score = get_best_match_score(file_name, pattern, &matcher, match_mode);
            }

            // Use config to check if file is content searchable
            if let Some(pattern) = content_pattern {
                if !is_dir && config.is_content_searchable(&path) {
                    if let Ok(matches) = search_file_content(&path, pattern, match_mode) {
                        if !matches.is_empty() {
                            content_matches = matches;
                        }
                    }
                }
            }

            // **NEW: Determine if this is a match and calculate combined score**
            let (is_match, final_score) = match search_type {
                SearchType::FileName => (filename_score.is_some(), filename_score.unwrap_or(0)),
                SearchType::Content => (!content_matches.is_empty(), if !content_matches.is_empty() { 100 } else { 0 }),
                SearchType::Hybrid => {
                    let has_filename = filename_score.is_some();
                    let has_content = !content_matches.is_empty();
                    let score = filename_score.unwrap_or(0) + if has_content { 50 } else { 0 };
                    (has_filename || has_content, score)
                }
            };

            if !is_match {
                return None;
            }

            // Count matched files/dirs
            if is_dir {
                dirs_scanned.fetch_add(1, Ordering::Relaxed);
            } else {
                files_scanned.fetch_add(1, Ordering::Relaxed);
            }

            let (size, modified) = if show_details ||
                config.output_options.show_details {
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

            Some(SearchResult {
                path,
                score: final_score,
                is_dir,
                size,
                modified,
                content_matches,
                search_type: search_type.clone(),
            })
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


pub fn get_best_match_score(
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