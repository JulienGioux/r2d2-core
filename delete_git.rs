use std::fs;

fn main() {
    let crates = ["r2d2-kernel", "r2d2-secure-mem", "r2d2-jsonai", "r2d2-paradox"];
    for c in &crates {
        let git_path = format!("{}/.git", c);
        
        // Supprime le dossier s'il existe. Résout le problème NTFS/WSL.
        if fs::metadata(&git_path).is_ok() {
            match fs::remove_dir_all(&git_path) {
                Ok(_) => println!("Deleted {}", git_path),
                Err(e) => println!("Failed to delete {}: {}", git_path, e),
            }
        }
    }
}
