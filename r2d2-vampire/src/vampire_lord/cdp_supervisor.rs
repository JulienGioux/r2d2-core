use chromiumoxide::Browser;
use r2d2_browser::{BrowserError, SovereignBrowser};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tracing::{error, info, warn};

/// Message entrant pour le superviseur CDP
pub enum SupervisorCommand {
    AskExpert {
        expert_name: String,
        target_url: String,
        prompt: String,
        responder: oneshot::Sender<anyhow::Result<String>>,
    },
}

/// Canal asynchrone global vers le superviseur (Actor Mailbox)
static SUPERVISOR_TX: OnceLock<mpsc::Sender<SupervisorCommand>> = OnceLock::new();

pub fn get_supervisor() -> mpsc::Sender<SupervisorCommand> {
    SUPERVISOR_TX
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel(32);
            tokio::spawn(async move {
                let mut supervisor = VampireSupervisor::new(rx);
                supervisor.run().await;
            });
            tx
        })
        .clone()
}

/// Le véritable Superviseur OTP (Erlang-Style) sans aucun Mutex global.
struct VampireSupervisor {
    rx: mpsc::Receiver<SupervisorCommand>,
    workers: HashMap<String, mpsc::Sender<SupervisorCommand>>,
    join_set: JoinSet<(String, anyhow::Result<()>)>,
    shared_browser: Option<Arc<Browser>>,
}

impl VampireSupervisor {
    fn new(rx: mpsc::Receiver<SupervisorCommand>) -> Self {
        Self {
            rx,
            workers: HashMap::new(),
            join_set: JoinSet::new(),
            shared_browser: None,
        }
    }

    async fn get_or_connect_browser(&mut self) -> Result<Arc<Browser>, BrowserError> {
        if let Some(b) = &self.shared_browser {
            return Ok(b.clone());
        }
        info!("⏳ Superviseur: Première connexion mutualisée à Chromium...");
        let b = SovereignBrowser::connect("chrome-profile").await?;
        let arc_b = Arc::new(b);
        self.shared_browser = Some(arc_b.clone());
        Ok(arc_b)
    }

    async fn run(&mut self) {
        info!("👁️ VampireSupervisor (OTP) démarré. Prêt à forker les Acteurs CDP.");
        loop {
            tokio::select! {
                cmd = self.rx.recv() => {
                    if let Some(command) = cmd {
                        self.handle_command(command).await;
                    } else {
                        warn!("VampireSupervisor: Canal principal fermé, arrêt du superviseur.");
                        break;
                    }
                }
                res = self.join_set.join_next(), if !self.join_set.is_empty() => {
                    self.handle_worker_exit(res);
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: SupervisorCommand) {
        let SupervisorCommand::AskExpert {
            expert_name,
            target_url,
            prompt,
            responder,
        } = cmd;
        let name = expert_name.clone();

        // On crée l'acteur de l'expert s'il n'existe pas.
        if !self.workers.contains_key(&name) {
            let browser = match self.get_or_connect_browser().await {
                Ok(b) => b,
                Err(e) => {
                    let _ = responder.send(Err(anyhow::anyhow!("Erreur Browser partagé: {}", e)));
                    return;
                }
            };

            let (tx, worker_rx) = mpsc::channel(10);
            self.workers.insert(name.clone(), tx);

            let n2 = name.clone();
            self.join_set.spawn(async move {
                let res =
                    crate::tools::notebook_lm::chrome_actor_loop(worker_rx, n2.clone(), browser)
                        .await;
                (n2, res)
            });
            info!("👷 Superviseur: Acteur CDP Spawné pour '{}'", name);
        }

        // Transmission au Worker
        if let Some(worker_tx) = self.workers.get(&name) {
            let fwd = SupervisorCommand::AskExpert {
                expert_name,
                target_url,
                prompt,
                responder,
            };
            if worker_tx.send(fwd).await.is_err() {
                // Le worker est mort juste avant / backpressure pleine. On nettoiera au join_next.
                error!(
                    "Superviseur: Le worker '{}' n'a pas pu recevoir le message.",
                    name
                );
            }
        }
    }

    fn handle_worker_exit(
        &mut self,
        res: Option<Result<(String, anyhow::Result<()>), tokio::task::JoinError>>,
    ) {
        if let Some(res) = res {
            match res {
                Ok((expert_name, result)) => {
                    self.workers.remove(&expert_name);
                    if let Err(e) = result {
                        error!(
                            "☠️ Superviseur: Acteur '{}' a crashé: {}. Auto-Heal préparé.",
                            expert_name, e
                        );
                    } else {
                        info!(
                            "🧹 Superviseur: Acteur '{}' s'est arrêté proprement.",
                            expert_name
                        );
                    }
                }
                Err(join_err) => {
                    error!(
                        "☠️ Superviseur: Tâche Acteur annulée ou a paniqué: {}",
                        join_err
                    );
                    // S'il panique, impossible de savoir son nom proprement, on peut le purger si c'était le seul
                    // mais le Rust pur ne panique pas.
                }
            }
        }
    }
}
