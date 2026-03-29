use anyhow::Result;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{info, instrument, warn};

/// ============================================================================
/// 🦾 PHYSICAL EXECUTOR ADAPTER (SYSTÈME 1 ACTUATOR)
/// ============================================================================
/// Exécute les actions réflexives d'urgence générées par le ReflexJudge.
/// Fonctionne au sein d'une Sandbox isolée (Podman/Docker) sans accès
/// au réseau hôte ni à l'environnement pour empêcher 100% des exfiltrations.
pub struct PhysicalExecutorAdapter {
    runtime_bin: String,
    timeout_secs: u64,
}

impl PhysicalExecutorAdapter {
    /// Crée l'Actuator avec le moteur de conteneurisation donné (ex: "docker" ou "podman").
    pub fn new(runtime_bin: &str, timeout_secs: u64) -> Self {
        Self {
            runtime_bin: runtime_bin.to_string(),
            timeout_secs,
        }
    }

    /// Exécute la directive réflexe d'urgence dans une sandbox ultra-sécurisée.
    #[instrument(skip(self, action_payload))]
    pub async fn execute_reflex(&self, action_payload: &str) -> Result<()> {
        info!(
            "🚨 ACTIVATION DU BRAS ACTUATEUR (Sandbox: {}) : {}",
            self.runtime_bin, action_payload
        );

        // 1. ZÉRO-TRUST PURE :
        // Le container monte une image alpine minimale, ferme le réseau, et n'a aucun montage volume hôte.
        // La consigne est passée via `sh -c`.
        let mut cmd = Command::new(&self.runtime_bin);
        cmd.args([
            "run",
            "--rm",      // Destruction éphémère immédiate après run
            "--network", // Empêche le ping/exfiltration
            "none",
            "--memory=64m", // Hard-limit mémoire
            "--cpus=0.5",   // Hard-limit CPU pour empêcher les boucles infinies de DOS le Kernel
            "alpine:latest",
            "sh",
            "-c",
            action_payload, // Le payload dynamique (ex: 'echo HALT')
        ])
        .env_clear() // Vide la table locale pour que même le subprocess Rust n'ait rien du parent.
        // Optionnel : un PATH restrictif vers les binaires locaux, mais on sort via docker/podman de toute façon
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        // 2. Spawn le process sandboxé
        let child_result = cmd.spawn();

        let mut child = match child_result {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Échec de spawn du moteur {}: {}", self.runtime_bin, e);
                tracing::error!("{}", msg);
                return Err(anyhow::anyhow!(msg));
            }
        };

        // 3. COUPE-CIRCUIT (Protection temporelle stricte)
        match timeout(Duration::from_secs(self.timeout_secs), child.wait()).await {
            Ok(Ok(status)) if status.success() => {
                info!("✅ [ACTUATOR] Réflexe exécuté et contenu avec succès.");
                Ok(())
            }
            Ok(Ok(status)) => {
                warn!(
                    "⚠️ [ACTUATOR] Le réflexe a échoué dans la sandbox (Exit {})",
                    status
                );
                // On peut potentiellement capturer stdout/stderr si besoin, mais pour le Système 1, l'action est expédiée.
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::error!(
                    "❌ [ACTUATOR] Erreur d'I/O lors de l'attente du conteneur: {}",
                    e
                );
                Err(anyhow::anyhow!("I/O Error: {}", e))
            }
            Err(_) => {
                warn!(
                    "⏳ [ACTUATOR] TIMEOUT (>{:?}). Le réflexe a été ABATTU manu-militari.",
                    self.timeout_secs
                );
                let _ = child.kill().await;
                Ok(()) // Le timeout d'un réflexe n'est pas une KernelError, c'est une intervention de sécurité normale.
            }
        }
    }
}
