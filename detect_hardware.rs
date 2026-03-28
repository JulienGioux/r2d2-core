use std::fs;
use std::process::Command;

fn main() {
    let mut report = String::new();

    report.push_str("--- CPU INFO ---\n");
    if let Ok(cpu) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpu.lines() {
            if line.starts_with("model name") {
                report.push_str(line);
                report.push('\n');
                break;
            }
        }
    }

    report.push_str("\n--- RAM INFO (WSL) ---\n");
    if let Ok(mem) = fs::read_to_string("/proc/meminfo") {
        for line in mem.lines().take(3) {
            report.push_str(line);
            report.push('\n');
        }
    }

    report.push_str("\n--- VRAM INFO (nvidia-smi) ---\n");
    if let Ok(output) = Command::new("nvidia-smi").output() {
        report.push_str(&String::from_utf8_lossy(&output.stdout));
    } else if let Ok(output) = Command::new("/mnt/c/Windows/System32/nvidia-smi.exe").output() {
        report.push_str(&String::from_utf8_lossy(&output.stdout));
    } else {
        report.push_str("nvidia-smi introuvable.\n");
    }

    fs::write("hardware_report.txt", report).unwrap();
}
