use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// --- Data Structures ---

#[derive(Serialize, Deserialize, Debug)]
struct Manifest {
    version_id: usize,
    timestamp: String,
    files: HashMap<String, String>, // Filename -> SHA256 Hash
}

const SCM_DIR: &str = ".scm";
const COMMITS_DIR: &str = "commits";
const HEAD_FILE: &str = "HEAD";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage:");
        println!("  scm commit   - Save current state");
        println!("  scm revert   - Revert to previous state");
        return;
    }

    match args[1].as_str() {
        "commit" => do_commit(),
        "revert" => do_revert(),
        _ => println!("Unknown command. Use 'commit' or 'revert'."),
    }
}

// --- Core Logic ---

fn do_commit() {
    init_repo_if_needed();

    let current_head = get_head();
    let new_id = current_head + 1;
    let new_commit_path = get_commit_path(new_id);

    fs::create_dir_all(&new_commit_path).expect("Failed to create commit dir");
    println!("Committing version {}...", new_id);

    let mut file_map = HashMap::new();
    let entries = fs::read_dir(".").expect("Failed to read current dir");

    for entry in entries {
        let entry = entry.expect("Error reading entry");
        let path = entry.path();
        
        if should_ignore(&path) { continue; }

        if path.is_file() {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let hash = calculate_hash(&path);
            
            let dest_path = new_commit_path.join(&filename);
            fs::copy(&path, &dest_path).expect("Failed to copy file");
            
            file_map.insert(filename, hash);
        }
    }

    let manifest = Manifest {
        version_id: new_id,
        timestamp: chrono::Utc::now().to_string(),
        files: file_map,
    };

    let manifest_path = new_commit_path.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(manifest_path, json).expect("Failed to write manifest");

    set_head(new_id);
    println!("Successfully committed version {}.", new_id);
}

fn do_revert() {
    if !Path::new(SCM_DIR).exists() {
        println!("No SCM repository found.");
        return;
    }

    let current_head = get_head();
    if current_head <= 1 {
        println!("Nothing to revert (already at initial state or empty).");
        return;
    }

    let target_id = current_head - 1;
    let target_path = get_commit_path(target_id);

    if !target_path.exists() {
        println!("Target version {} not found.", target_id);
        return;
    }

    println!("Reverting to version {}...", target_id);

    let manifest_path = target_path.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path).expect("Missing manifest");
    let manifest: Manifest = serde_json::from_str(&manifest_content).expect("Invalid manifest");

    // Integrity Check
    for (filename, recorded_hash) in &manifest.files {
        let file_path = target_path.join(filename);
        if !file_path.exists() { panic!("INTEGRITY ERROR: Backup file missing!"); }
        let current_hash = calculate_hash(&file_path);
        if &current_hash != recorded_hash { panic!("INTEGRITY ERROR: Backup corrupted!"); }
    }
    println!("Integrity check passed. Restoring files...");

    // Clear current files
    let entries = fs::read_dir(".").unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if !should_ignore(&path) && path.is_file() {
            fs::remove_file(path).expect("Failed to delete current file");
        }
    }

    // Restore
    for (filename, _) in &manifest.files {
        let src = target_path.join(filename);
        let dest = Path::new(filename);
        fs::copy(src, dest).expect("Failed to restore file");
    }

    set_head(target_id);
    println!("Revert complete. Now at version {}.", target_id);
}

// --- Helpers ---

fn init_repo_if_needed() {
    let scm_path = Path::new(SCM_DIR);
    if !scm_path.exists() {
        fs::create_dir(scm_path).expect("Failed to create .scm dir");
        let commits_path = scm_path.join(COMMITS_DIR);
        fs::create_dir(&commits_path).expect("Failed to create commits dir");
        set_head(0);
        println!("Initialized empty SCM repository.");
    }
}

fn get_commit_path(id: usize) -> PathBuf {
    Path::new(SCM_DIR).join(COMMITS_DIR).join(id.to_string())
}

fn get_head() -> usize {
    let head_path = Path::new(SCM_DIR).join(HEAD_FILE);
    if !head_path.exists() { return 0; }
    let content = fs::read_to_string(head_path).unwrap_or("0".to_string());
    content.trim().parse().unwrap_or(0)
}

fn set_head(id: usize) {
    fs::write(Path::new(SCM_DIR).join(HEAD_FILE), id.to_string()).expect("Failed to write HEAD");
}

fn calculate_hash(path: &Path) -> String {
    let mut file = fs::File::open(path).expect("Failed to open file");
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).expect("Failed to read file");
    hex::encode(hasher.finalize())
}

fn should_ignore(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains(".scm") || s.contains(".git") || s.contains("target") || s.ends_with("scm") || s.ends_with(".rs") || s.contains("Cargo")
}