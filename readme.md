# **I gave up**

# ff - Fast File

**`ff`** is a *blazing-fast⚡* lightweight command-line tool for **finding files** and **directories** on your filesystem.
Just type part of a name, and ff helps you quickly **find** it—then open it in your *favourite IDE* or *editor*,
or maybe this serves different purpose, I am **not** aware.

[![Rust](https://img.shields.io/badge/rust-000000?style=for-the-badge&logo=rust&logoColor=white)](https://rust-lang.org/)
![Rust](https://img.shields.io/badge/rust-1.80+-orange.svg)


## ![Demo](demo-gif.gif)  
**Search and open your favourite IDE!**

## Features

*   **Selection:**
    *   Copy the selected file path to the clipboard (`--copy`).
    *   Open a new terminal in the selected file's directory (`--terminal`).
*   **Filtering:**
    *   Search for directories only (`--dirs-only`).
    *   Search for files only (`--files-only`).
    *   Include hidden files and directories in your search (`--hidden`).
*   **Custom Search:**
    *   Specify a search path (`--path`).
    *   Limit the number of results (`--limit`).
    *   View detailed file information like size and modification date (`--details`).
*   **Cross-Platform:** Works on Windows, macOS, and Linux.

    ![Linux](https://img.shields.io/badge/Linux-Yes-blue?logo=linux)  ![Windows](https://img.shields.io/badge/Windows-Yes-blue?logo=windows)  ![Mac](https://img.shields.io/badge/macOS-Yes-blue?logo=apple)   


## Prerequisites
- Windows: [Rust](https://rustup.rs/) (1.88.0 or later)
- WSL (Run this 👇):
    ```bash 
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```


## Installation

You can build from source, locally using `cargo`:

```bash
git clone https://github.com/ronakgh97/file-find.git
cd file-find
cargo clean
cargo build --release
cargo install --path .
```

## Usage

Here are some examples of how to use `ff`:

```bash
# Search for "config.json" in the current directory
ff config.json

# Search for "main" in the /codes directory (using |⚡| search)
ff main --path /codes --parallel

# Find only directories matching "docker"
ff docker --dirs-only

# Find a file and copy its path to the clipboard
ff package.json --copy

# Find only Rust files (use quotes for wildcards)
ff "*.rs" --files-only
```

### Options

| Short | Long           | Description                                          |
|-------|----------------|------------------------------------------------------|
| `-p`  | `--path`       | Directory to search in (default: current directory)  |
| `-c`  | `--copy`       | Copy selected path to clipboard                      |
| `-h`  | `--hidden`     | Include hidden files and directories                 |
| `-l`  | `--limit`      | Maximum number of results to show (default: 10)      |
| `-d`  | `--dirs-only`  | Only match directories                               |
| `-f`  | `--files-only` | Only match files (exclude directories)               |
|       | `--details`    | Show detailed information (file sizes, dates)        |
| `-t`  | `--terminal`   | Open new terminal window in the selected directory   |
| `-m`  | `--match-mode` | Matching mode: `fuzzy` or `exact` (default: `fuzzy`) |
| `-pl` | `--parallel`   | uses optimal threads for fast searching              |

