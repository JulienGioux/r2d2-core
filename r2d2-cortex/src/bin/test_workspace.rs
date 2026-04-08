fn main() {
    println!("Testing PodmanWorkspace");
    let (_ws, _) =
        r2d2_cortex::workspace::PodmanWorkspace::new("r2d2-test-new", Some("fedora:latest"), None)
            .unwrap();
    println!("Done");
    let out = std::process::Command::new("podman")
        .args(["ps", "-a"])
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&out.stdout));
}
