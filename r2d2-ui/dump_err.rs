use std::process::Command;
use std::fs;

fn main() {
    let output = Command::new("cargo")
        .arg("check")
        .output()
        .expect("failed to execute process");

    let mut err = String::from_utf8_lossy(&output.stderr).to_string();
    let out = String::from_utf8_lossy(&output.stdout).to_string();

    fs::write("err.txt", format!("STDOUT:\n{}\n\nSTDERR:\n{}", out, err)).unwrap();
}
