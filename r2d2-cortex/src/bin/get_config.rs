use std::process::Command;

fn main() {
    println!("=== R2D2 CONFIGURATION EXTRACTOR ===");

    // RAM
    let ram_output = Command::new("cmd")
        .args(&["/C", "wmic OS get TotalVisibleMemorySize /Value"])
        .output()
        .expect("Failed to execute cmd");
    println!("RAM:\n{}", String::from_utf8_lossy(&ram_output.stdout));

    // VRAM / GPU
    let vram_output = Command::new("cmd")
        .args(&["/C", "wmic path win32_VideoController get name, adapterram"])
        .output()
        .expect("Failed to execute cmd");
    println!(
        "GPU / VRAM:\n{}",
        String::from_utf8_lossy(&vram_output.stdout)
    );

    // CPU
    let cpu_output = Command::new("cmd")
        .args(&["/C", "wmic cpu get name"])
        .output()
        .expect("Failed to execute cmd");
    println!("CPU:\n{}", String::from_utf8_lossy(&cpu_output.stdout));
}
