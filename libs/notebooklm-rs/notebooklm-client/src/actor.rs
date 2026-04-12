use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

use notebooklm_core::errors::{NotebookError, Result};
use r2d2_browser::SovereignBrowser;

/// Commandes asynchrones envoyées à l'Acteur Chromium
#[derive(Debug)]
pub enum BrowserCommand {
    ExecuteScript {
        script: String,
        respond_to: oneshot::Sender<Result<serde_json::Value>>,
    },
    ReloadSession {
        respond_to: oneshot::Sender<Result<()>>,
    },
}

/// L'Acteur Chromium, gérant la possession stricte de la session
pub struct ChromiumActor {
    receiver: mpsc::Receiver<BrowserCommand>,
    tab: Option<Arc<chromiumoxide::Page>>,
}

impl ChromiumActor {
    /// Lance l'acteur dans une tâche autonome (Thread non-starvant)
    pub fn spawn(capacity: usize) -> mpsc::Sender<BrowserCommand> {
        let (tx, rx) = mpsc::channel(capacity);
        
        let actor = ChromiumActor {
            receiver: rx,
            tab: None,
        };

        // Lancement natif Tokio
        tokio::spawn(async move {
            actor.run().await;
        });

        tx
    }

    /// Boucle de vie principale de l'Acteur
    async fn run(mut self) {
        // Tentative d'initialisation au démarrage
        if let Err(e) = self.init_session().await {
            error!("Échec de l'initialisation du navigateur au démarrage de l'acteur: {}", e);
        }

        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                BrowserCommand::ExecuteScript { script, respond_to } => {
                    let res = self.handle_execute_script(&script).await;
                    let _ = respond_to.send(res);
                }
                BrowserCommand::ReloadSession { respond_to } => {
                    let res = self.init_session().await;
                    let _ = respond_to.send(res);
                }
            }
        }
        info!("ChromiumActor mailbox fermée, arrêt propre.");
    }

    /// Initialise ou réinitialise la session CDP avec la logique de Retry
    async fn init_session(&mut self) -> Result<()> {
        info!("ChromiumActor: Initialisation de la session Sovereign...");
        
        // Mettre en place un retry basique 
        let mut attempts = 0;
        loop {
            attempts += 1;
            match SovereignBrowser::connect("Chrome_GOOGLE").await {
                Ok(browser) => {
                    match SovereignBrowser::get_or_new_tab(&browser, "notebooklm.google.com").await {
                        Ok(tab) => {
                            self.tab = Some(tab);
                            info!("ChromiumActor: Session NotebookLM connectée avec succès.");
                            return Ok(());
                        }
                        Err(e) => {
                            if attempts >= 3 {
                                return Err(NotebookError::InfrastructureError(format!("Impossible d'ouvrir l'onglet: {}", e)));
                            }
                        }
                    }
                }
                Err(e) => {
                    if attempts >= 3 {
                        return Err(NotebookError::InfrastructureError(format!("Impossible de se connecter au navigateur: {}", e)));
                    }
                }
            }
            warn!("Échec de connexion au navigateur (essai {}/3). Retentative dans 2s...", attempts);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    /// Exécute un script avec File d'attente
    async fn handle_execute_script(&mut self, script: &str) -> Result<serde_json::Value> {
        if self.tab.is_none() {
            // Tente de se reconnecter si le tab est mort
            self.init_session().await?;
        }

        if let Some(tab) = &self.tab {
            match tab.evaluate(script).await {
                Ok(eval) => {
                    if let Some(value) = eval.value() {
                        Ok(value.clone())
                    } else {
                        Err(NotebookError::PayloadParsingError("Valeur JS nulle renvoyée".to_string()))
                    }
                }
                Err(e) => {
                    warn!("Erreur lors de l'évaluation du script, drop du tab: {}", e);
                    self.tab = None; // Force reload au prochain appel
                    Err(NotebookError::InfrastructureError(e.to_string()))
                }
            }
        } else {
            Err(NotebookError::InfrastructureError("Navigateur non disponible".to_string()))
        }
    }
}
