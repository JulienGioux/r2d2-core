use std::process::Command;
use std::fs;

fn main() {
    // 1. Ensure all .git folders are completely removed
    let crates = ["r2d2-kernel", "r2d2-secure-mem", "r2d2-jsonai", "r2d2-paradox"];
    for c in &crates {
        let git_path = format!("{}/.git", c);
        let _ = fs::remove_dir_all(&git_path);
    }

    // 2. Remove submodules from git cache (ignores errors if not a submodule)
    for c in &crates {
        let _ = Command::new("git")
            .args(&["rm", "--cached", "-r", c])
            .output();
    }

    // 3. Add the files correctly
    for c in &crates {
        let status = Command::new("git")
            .args(&["add", c])
            .status()
            .expect("Failed to execute git add");
        
        if !status.success() {
            println!("Failed to add {}", c);
        }
    }

    // 4. Commit the changes
    let status = Command::new("git")
        .args(&["commit", "-m", "fix: properly track cargo workspace crates as source files"])
        .status()
        .expect("Failed to execute git commit");

    if status.success() {
        println!("Git index successfully repaired and committed.");
    } else {
        println!("Commit returned non-zero (maybe no changes)");
    }
}
