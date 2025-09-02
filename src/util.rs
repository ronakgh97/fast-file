use std::io;
use std::io::Write;
use std::path::Path;
use colored::Colorize;
use crate::{SearchResult, SearchType};

// Update display_results in util.rs
pub fn display_results(results: &[SearchResult], show_details: bool) {
    if results.is_empty() {
        println!();
        println!("{}", "No files found matching the pattern".bright_red());
        return;
    }

    println!();
    println!("{} Found {} match(es):", "✅".green(), results.len().to_string().bright_green().bold());

    for (index, result) in results.iter().enumerate() {
        println!();
        let index_str = format!("{:2}", index + 1);
        let type_icon = get_file_icon(result);
        let path_str = result.path.display().to_string();

        let mut line = format!(
            "{} {} {}",
            index_str.bright_blue().bold(),
            type_icon,
            path_str.white(),
        );

        // Add search type indicator
        match result.search_type {
            SearchType::Content => line.push_str(&format!(" {}", "[CONTENT]".green())),
            SearchType::Hybrid => line.push_str(&format!(" {}", "[HYBRID]".yellow())),
            _ => {}
        }

        if show_details {
            if let Some(size) = result.size {
                line.push_str(&format!(" {}", format_size(size).dimmed()));
            }
            if let Some(modified) = result.modified {
                if let Ok(elapsed) = modified.elapsed() {
                    line.push_str(&format!(" {}", format_time_ago(elapsed).dimmed()));
                }
            }
            line.push_str(&format!(" {}", format!("({})", result.score).bright_black()));
        }

        println!("{}", line);

        // Show content matches
        if !result.content_matches.is_empty() {
            for (i, content_match) in result.content_matches.iter().enumerate() {
                if i >= 3 { // Limit to first 3 matches per file
                    println!("    {} {} more matches...", "...".dimmed(), (result.content_matches.len() - 3).to_string().dimmed());
                    break;
                }

                let line_preview = if content_match.line_content.len() > 100 {
                    format!("{}...", &content_match.line_content[..97])
                } else {
                    content_match.line_content.clone()
                };

                println!("    {}: {}",
                         format!("L{}", content_match.line_number).blue(),
                         line_preview.dimmed()
                );
            }
        }
    }
}


pub fn get_file_icon(result: &SearchResult) -> &'static str {
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

pub fn format_size(bytes: u64) -> String {
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

pub fn format_time_ago(elapsed: std::time::Duration) -> String {
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

pub fn get_file_metadata(entry: &walkdir::DirEntry) -> (Option<u64>, Option<std::time::SystemTime>) {
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


pub fn interactive_select(results: &[SearchResult]) -> Option<&SearchResult> {
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

pub fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    println!();
    println!("{} Path copied to clipboard:", "📋".green());
    println!("   {}", text.cyan());
    Ok(())
}

pub fn change_directory(path: &Path) {
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

pub fn spawn_terminal(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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