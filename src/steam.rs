use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Structure representing a Steam game.
#[derive(Debug, Clone)]
pub struct SteamGame {
    pub appid: String,
    pub name: String,
}

/// The AppState data inside the manifest file.
#[derive(Deserialize, Debug)]
struct AppState {
    appid: String,
    name: String,
}

/// Locates the steamapps directory by checking standard user paths.
/// Checks:
/// 1. ~/.local/share/Steam/steamapps/
/// 2. ~/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/ (Flatpak Steam)
pub fn get_steamapps_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home_str) = std::env::var("HOME") {
        let home = PathBuf::from(home_str);
        
        // Standard path
        let standard_path = home.join(".local/share/Steam/steamapps");
        if standard_path.is_dir() {
            dirs.push(standard_path);
        }
        
        // Flatpak path
        let flatpak_path = home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps");
        if flatpak_path.is_dir() {
            dirs.push(flatpak_path);
        }
    }
    dirs
}

/// Scans the located steamapps directories for all appmanifest_*.acf files,
/// deserializing each VDF file to retrieve game information.
pub fn scan_steam_games() -> Vec<SteamGame> {
    let mut games = Vec::new();
    let dirs = get_steamapps_dirs();

    for dir in dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                        // Parse only files matching 'appmanifest_*.acf'
                        if file_name.starts_with("appmanifest_") && file_name.ends_with(".acf") {
                            if let Ok(content) = fs::read_to_string(&file_path) {
                                match keyvalues_serde::from_str::<AppState>(&content) {
                                    Ok(manifest) => {
                                        games.push(SteamGame {
                                            appid: manifest.appid,
                                            name: manifest.name,
                                        });
                                    }
                                    Err(err) => {
                                        eprintln!("Failed to parse manifest {:?}: {}", file_path, err);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate games in case they are present in multiple searched directories
    games.sort_by_key(|g| g.appid.clone());
    games.dedup_by(|a, b| a.appid == b.appid);

    // Sort alphabetically by name
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    
    games
}

/// Launches a steam game using Steam's custom URI protocol.
/// Spawns 'steam steam://run/{appid}' as a detached child process.
/// Cascades to 'xdg-open' as a fallback if the 'steam' command is not in the system path.
pub fn launch_game(appid: &str) -> std::io::Result<()> {
    let uri = format!("steam://run/{}", appid);
    println!("Spawning detached process to launch game with AppID: {}", appid);

    // Launch steam directly with the URI
    match Command::new("steam").arg(&uri).spawn() {
        Ok(_) => {
            println!("Successfully spawned native steam URI command.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to spawn native steam launcher command: {}. Trying fallback with xdg-open...", e);
            // Fallback to xdg-open which uses standard desktop system handlers to invoke the registered steam handler
            Command::new("xdg-open")
                .arg(&uri)
                .spawn()
                .map(|_| ())
        }
    }
}

/// Triggers installation of a Steam game by opening 'steam://install/{appid}'.
pub fn install_game(appid: &str) -> std::io::Result<()> {
    let uri = format!("steam://install/{}", appid);
    println!("Spawning process to trigger install for AppID: {}", appid);

    match Command::new("steam").arg(&uri).spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Failed to spawn native steam launcher command for install: {}. Trying fallback with xdg-open...", e);
            Command::new("xdg-open")
                .arg(&uri)
                .spawn()
                .map(|_| ())
        }
    }
}

/// Triggers uninstallation of a Steam game by opening 'steam://uninstall/{appid}'.
pub fn uninstall_game(appid: &str) -> std::io::Result<()> {
    let uri = format!("steam://uninstall/{}", appid);
    println!("Spawning process to trigger uninstall for AppID: {}", appid);

    match Command::new("steam").arg(&uri).spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Failed to spawn native steam launcher command for uninstall: {}. Trying fallback with xdg-open...", e);
            Command::new("xdg-open")
                .arg(&uri)
                .spawn()
                .map(|_| ())
        }
    }
}

/// Item structure returned by Steam store search API.
#[derive(serde::Deserialize, Debug, Clone)]
pub struct SearchResultItem {
    pub id: u64,
    pub name: String,
}

/// Search API JSON response wrapper.
#[derive(serde::Deserialize, Debug)]
struct SearchResponse {
    items: Vec<SearchResultItem>,
}

/// Searches the Steam storefront for games matching the provided term.
pub fn search_games(query: &str) -> std::io::Result<Vec<SearchResultItem>> {
    let encoded = query.replace(" ", "+");
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        encoded
    );

    println!("Searching Steam store with query: {}", query);
    let output = Command::new("curl")
        .arg("-s")
        .arg(&url)
        .output()?;

    if output.status.success() {
        match serde_json::from_slice::<SearchResponse>(&output.stdout) {
            Ok(response) => Ok(response.items),
            Err(e) => {
                eprintln!("JSON parsing error from Steam search: {}", e);
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
        }
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "curl execution failed"))
    }
}

/// Locates a cached game cover image inside Steam's appcache/librarycache.
/// Only matches vertical poster images ('library_600x900.jpg') to maintain grid uniformity.
pub fn find_game_cover(appid: &str) -> Option<PathBuf> {
    let home_str = std::env::var("HOME").ok()?;
    let home = PathBuf::from(home_str);
    
    // Check both host and Flatpak locations
    let base_paths = [
        home.join(".local/share/Steam/appcache/librarycache"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/appcache/librarycache"),
    ];
    
    for base in &base_paths {
        let app_dir = base.join(appid);
        if app_dir.is_dir() {
            // 1. Check direct poster cover
            let direct_cover = app_dir.join("library_600x900.jpg");
            if direct_cover.is_file() {
                return Some(direct_cover);
            }
            
            // 2. Recursive search fallback for vertical poster
            if let Ok(entries) = walk_dir_for_covers(&app_dir) {
                if !entries.is_empty() {
                    return Some(entries[0].clone());
                }
            }
        }
    }
    None
}

/// Helper to recursively search for files named 'library_600x900.jpg'.
fn walk_dir_for_covers(dir: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut sub_files) = walk_dir_for_covers(&path) {
                    files.append(&mut sub_files);
                }
            } else if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "library_600x900.jpg" {
                        files.push(path);
                    }
                }
            }
        }
    }
    Ok(files)
}

/// Checks if the Steam client process is running.
/// Scans the `/proc` directory for a process named `steam`.
pub fn is_steam_running() -> bool {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.chars().all(|c| c.is_ascii_digit()) {
                    let comm_path = entry.path().join("comm");
                    if let Ok(comm) = std::fs::read_to_string(comm_path) {
                        let comm_trimmed = comm.trim();
                        if comm_trimmed == "steam" {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Launches the Steam client in the background.
pub fn launch_steam() -> std::io::Result<()> {
    println!("Spawning Steam client process in the background.");
    match Command::new("steam").spawn() {
        Ok(_) => Ok(()),
        Err(_) => {
            // Fallback for flatpak Steam
            Command::new("flatpak")
                .arg("run")
                .arg("com.valvesoftware.Steam")
                .spawn()
                .map(|_| ())
        }
    }
}

/// Stats parsed from Steam's localconfig.vdf
#[derive(Debug, Clone)]
pub struct GameStats {
    pub playtime_mins: u32,
    pub last_played_timestamp: u64,
}

/// Locates and parses localconfig.vdf for the game's playtime and last played date.
pub fn get_game_stats(appid: &str) -> Option<GameStats> {
    let home_str = std::env::var("HOME").ok()?;
    let home = PathBuf::from(home_str);

    let base_paths = [
        home.join(".local/share/Steam/userdata"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/userdata"),
    ];

    for base in &base_paths {
        if !base.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let localconfig_path = path.join("config/localconfig.vdf");
                    if localconfig_path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&localconfig_path) {
                            if let Some(stats) = parse_stats_from_vdf(&content, appid) {
                                return Some(stats);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Fast string-scanning helper to extract playtime and last-played values.
fn parse_stats_from_vdf(content: &str, appid: &str) -> Option<GameStats> {
    let target_key = format!("\"{}\"", appid);
    let mut lines = content.lines();

    while let Some(line) = lines.next() {
        if line.trim() == target_key {
            if let Some(open_brace_line) = lines.next() {
                if open_brace_line.trim() == "{" {
                    let mut playtime = 0;
                    let mut last_played = 0;

                    while let Some(inner_line) = lines.next() {
                        let trimmed = inner_line.trim();
                        if trimmed == "}" {
                            break;
                        }
                        if trimmed.starts_with("\"LastPlayed\"") {
                            if let Some(val) = extract_vdf_value(trimmed) {
                                last_played = val.parse::<u64>().unwrap_or(0);
                            }
                        } else if trimmed.starts_with("\"Playtime\"") {
                            if let Some(val) = extract_vdf_value(trimmed) {
                                playtime = val.parse::<u32>().unwrap_or(0);
                            }
                        }
                    }

                    return Some(GameStats {
                        playtime_mins: playtime,
                        last_played_timestamp: last_played,
                    });
                }
            }
        }
    }
    None
}

fn extract_vdf_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 {
        Some(parts[3].to_string())
    } else {
        None
    }
}



