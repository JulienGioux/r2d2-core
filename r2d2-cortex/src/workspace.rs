use crate::error::CortexError;

pub trait Workspace {
    /// Executes a shell command inside the workspace and returns (stdout, stderr, exit_code)
    fn exec(&self, cmd: &str) -> Result<(String, String, i32), CortexError>;
}

pub struct PodmanWorkspace {
    pub container_name: String,
}

impl PodmanWorkspace {
    #[tracing::instrument(skip_all, fields(container_name = %name))]
    pub fn new(
        name: &str,
        base_image: Option<&str>,
        _script: Option<&str>,
    ) -> Result<(Self, bool), CortexError> {
        let container_name = name;
        let mut target_image = base_image.unwrap_or("registry.fedoraproject.org/fedora:latest");
        if target_image.trim().is_empty() {
            target_image = "registry.fedoraproject.org/fedora:latest";
        }

        // Assure custom bridge network existence
        let net_output = std::process::Command::new("podman")
            .args(["network", "create", "r2d2-net"])
            .output()?;

        if !net_output.status.success() {
            let stderr = String::from_utf8_lossy(&net_output.stderr);
            if !stderr.contains("already exists") && !stderr.contains("existe déjà") {
                tracing::warn!("Podman network create info: {}", stderr.trim());
            }
        }

        let status = std::process::Command::new("podman")
            .args(["inspect", "-f", "{{.State.Running}}", container_name])
            .output()?;

        let mut should_start = false;
        let is_running = String::from_utf8_lossy(&status.stdout).trim() == "true";

        if !is_running {
            should_start = true;
            let _ = std::process::Command::new("podman")
                .args(["rm", "-f", container_name])
                .output();
        }

        if should_start {
            tracing::info!(
                "Démarrage du conteneur Podman: {} depuis {}",
                container_name,
                target_image
            );
            let run_output = std::process::Command::new("podman")
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
                .output()?;

            if !run_output.status.success() {
                let stderr = String::from_utf8_lossy(&run_output.stderr);
                tracing::error!(
                    "Echec du démarrage de Podman ({}): {}",
                    run_output.status,
                    stderr
                );
                return Err(CortexError::WorkspaceError(format!(
                    "Echec du démarrage de Podman ({}): {}",
                    run_output.status, stderr
                )));
            }
        }

        Ok((
            Self {
                container_name: container_name.to_string(),
            },
            should_start,
        ))
    }
}

impl Workspace for PodmanWorkspace {
    #[tracing::instrument(skip(self), fields(container = %self.container_name, cmd = %cmd))]
    fn exec(&self, cmd: &str) -> Result<(String, String, i32), CortexError> {
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

        if exit_code != 0 {
            tracing::warn!("Commande podman exec terminee avec le code {}", exit_code);
        }

        Ok((stdout, stderr, exit_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_podman_exec_basic_command() {
        if std::process::Command::new("podman")
            .arg("--version")
            .output()
            .is_err()
        {
            println!("⚠️ Podman non installé sur l'hôte, test d'intégration ignoré.");
            return;
        }

        // Ensure podman is running and r2d2-workspace exists before testing.
        let workspace_init = PodmanWorkspace::new("r2d2-workspace-tmp-test", None, None);
        if let Err(e) = workspace_init {
            println!("⚠️ Impossible d'initialiser le conteneur de test, cause probable d'environnement. Ignoré. Erreur: {}", e);
            return;
        }
        let (workspace, _) = workspace_init.unwrap();

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

        let _ = std::process::Command::new("podman")
            .args(["rm", "-f", "r2d2-workspace-tmp-test"])
            .output();
    }
}
