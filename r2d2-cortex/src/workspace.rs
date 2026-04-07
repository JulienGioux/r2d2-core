use anyhow::Result;

pub trait Workspace {
    /// Executes a shell command inside the workspace and returns (stdout, stderr, exit_code)
    fn exec(&self, cmd: &str) -> Result<(String, String, i32)>;
}

pub struct PodmanWorkspace {
    pub container_name: String,
}

impl PodmanWorkspace {
    pub fn new(name: &str, base_image: Option<&str>, _script: Option<&str>) -> (Self, bool) {
        let container_name = name;
        let mut target_image = base_image.unwrap_or("registry.fedoraproject.org/fedora:latest");
        if target_image.trim().is_empty() {
            target_image = "registry.fedoraproject.org/fedora:latest";
        }

        // Assure custom bridge network existence
        let _ = std::process::Command::new("podman")
            .args(["network", "create", "r2d2-net"])
            .output();

        let status = std::process::Command::new("podman")
            .args(["inspect", "-f", "{{.State.Running}}", container_name])
            .output();

        let mut should_start = false;
        let is_running = match status {
            Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "true",
            Err(_) => false,
        };

        if !is_running {
            should_start = true;
            let _ = std::process::Command::new("podman")
                .args(["rm", "-f", container_name])
                .output();
        }

        if should_start {
            let _ = std::process::Command::new("podman")
                .args([
                    "run",
                    "-d",
                    "--name",
                    container_name,
                    "--hostname",
                    container_name,
                    "--network",
                    "r2d2-net",
                    target_image,
                    "tail",
                    "-f",
                    "/dev/null",
                ])
                .output();
        }

        (
            Self {
                container_name: container_name.to_string(),
            },
            should_start,
        )
    }
}

impl Workspace for PodmanWorkspace {
    fn exec(&self, cmd: &str) -> Result<(String, String, i32)> {
        let output = std::process::Command::new("podman")
            .arg("exec")
            .arg(&self.container_name)
            .arg("sh")
            .arg("-c")
            .arg(cmd)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_podman_exec_basic_command() {
        // Ensure podman is running and r2d2-workspace exists before testing.
        // TDD: This test will fail because exec() currently returns "Not implemented yet".
        let (workspace, _) = PodmanWorkspace::new("r2d2-workspace", None, None);

        let result = workspace.exec("echo 'hello sandbox'");
        assert!(
            result.is_ok(),
            "Exec should not return an error: {:?}",
            result.err()
        );

        let (stdout, stderr, exit_code) = result.unwrap();
        assert_eq!(exit_code, 0);
        assert_eq!(stdout.trim(), "hello sandbox");
        assert!(stderr.is_empty());
    }
}
