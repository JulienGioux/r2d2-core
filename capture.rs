use std::process::Command;
use std::fs;

fn main() {
    let output = Command::new("cargo")
        .args(&["clippy", "--workspace", "--all-targets", "--all-features", "--message-format=short"])
        .output()
        .expect("Failed to execute command");

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    let combined = format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout_str, stderr_str);
    fs::write("capture.log", combined).expect("Unable to write file");
}
