use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DefaultSearchOptions {
    pub match_mode: String,       // "fuzzy" or "exact"
    pub case_sensitive: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputOptions {
    pub show_details: bool,
    pub color_theme: String,
    pub max_content_matches: usize,
    pub max_line_length: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub ignore_directories: Vec<String>,
    pub ignore_file_patterns: Vec<String>,
    pub max_memory_mb: usize,
    pub max_files_per_search: usize,
    pub max_parallel_threads: Option<usize>,
    pub max_file_size_mb: u64,
    pub include_hidden: bool,
    pub follow_symlinks: bool,
    pub content_search_extensions: Vec<String>,
    pub default_search_options: DefaultSearchOptions,
    pub output_options: OutputOptions,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore_directories: vec![
                "node_modules".to_string(),
                "target".to_string(),
                "build".to_string(),
                ".git".to_string(),
                "AppData".to_string(),
                "Windows".to_string(),
                "System32".to_string(),
                "Cache".to_string(),
                "Temp".to_string(),
                ".cache".to_string(),
            ],
            ignore_file_patterns: vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                "*.bak".to_string(),
                "*.swp".to_string(),
                "thumbs.db".to_string(),
                ".DS_Store".to_string(),
            ],
            max_memory_mb: 1024,
            max_files_per_search: 50000,
            max_parallel_threads: None,  // Auto-detect
            max_file_size_mb: 10,
            include_hidden: false,
            follow_symlinks: false,
            content_search_extensions: vec![
                ".rs".to_string(),
                ".py".to_string(),
                ".js".to_string(),
                ".ts".to_string(),
                ".java".to_string(),
                ".cpp".to_string(),
                ".c".to_string(),
                ".h".to_string(),
                ".txt".to_string(),
                ".md".to_string(),
                ".json".to_string(),
                ".yaml".to_string(),
                ".yml".to_string(),
                ".toml".to_string(),
                ".cfg".to_string(),
            ],
            default_search_options: DefaultSearchOptions {
                match_mode: "fuzzy".to_string(),
                case_sensitive: false,
            },
            output_options: OutputOptions {
                show_details: true,
                color_theme: "default".to_string(),
                max_content_matches: 3,
                max_line_length: 100,
            },
        }
    }
}

impl Config {
    /// Main entry point - handles all config logic with safeguards
    pub fn load_with_safeguard() -> Self {
        let config_path = PathBuf::from("ff-config.json");

        if config_path.exists() {
            match Self::load_from_file(&config_path) {
                Ok(config) => {
                    println!("📁 Loaded config from: {}", config_path.display());
                    config
                },
                Err(_) => {
                    println!("⚠️  Invalid config file detected, regenerating default config");
                    let default_config = Self::default();
                    if let Err(e) = default_config.save_to_file(&config_path) {
                        println!("⚠️  Warning: Could not save config: {}", e);
                    }
                    default_config
                }
            }
        } else {
            println!("📁 Config file not found, creating default config");
            let default_config = Self::default();
            if let Err(e) = default_config.save_to_file(&config_path) {
                println!("⚠️  Warning: Could not save config: {}", e);
            }
            default_config
        }
    }

    /// Load config from specific file path
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save config to specific file path
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        println!("💾 Config saved to: {}", path.display());
        Ok(())
    }

    // Helper methods for using the config
    pub fn should_ignore_directory(&self, dir_name: &str) -> bool {
        self.ignore_directories.iter().any(|pattern| dir_name.contains(pattern))
    }

    pub fn should_ignore_file(&self, file_name: &str) -> bool {
        self.ignore_file_patterns.iter().any(|pattern| {
            if pattern.starts_with("*.") {
                let ext = &pattern[2..];
                file_name.ends_with(ext)
            } else {
                file_name.contains(pattern)
            }
        })
    }

    pub fn is_content_searchable(&self, file_path: &std::path::Path) -> bool {
        if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
            let ext_with_dot = format!(".{}", ext);
            self.content_search_extensions.contains(&ext_with_dot)
        } else {
            // Files without extension - check common names
            if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
                matches!(name, "README" | "Makefile" | "Dockerfile" | "LICENSE")
            } else {
                false
            }
        }
    }

    pub fn get_effective_thread_count(&self, cli_threads: Option<usize>, max_cpu_flag: bool) -> usize {
        if let Some(threads) = cli_threads {
            threads  // CLI override
        } else if let Some(config_threads) = self.max_parallel_threads {
            config_threads  // Config override
        } else if max_cpu_flag {
            num_cpus::get() * 2  // CLI max flag
        } else {
            num_cpus::get()  // Default
        }
    }
}