use std::time::Duration;
use sysinfo::System;
use tokio::sync::watch;
use tokio::task;

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMetrics {
    pub ram_usage_ratio: f32,
    pub cpu_usage_ratio: f32,
}

pub struct HardwareMonitor {
    pub receiver: watch::Receiver<SystemMetrics>,
}

impl HardwareMonitor {
    /// Lance un thread système isolé pour monitorer le Hardware.
    /// Cela évite de bloquer l'executor asynchrone Tokio avec des I/O OS lourdes (comme `refresh_all`).
    pub fn start() -> Self {
        let (sender, receiver) = watch::channel(SystemMetrics::default());

        task::spawn_blocking(move || {
            let mut sys = System::new_all();
            // Préchauffe le CPU une première fois (requis par sysinfo)
            sys.refresh_cpu_usage();
            std::thread::sleep(Duration::from_millis(200));

            loop {
                sys.refresh_memory();
                sys.refresh_cpu_usage();

                let total_ram = sys.total_memory() as f32;
                let used_ram = sys.used_memory() as f32;
                let ram_usage_ratio = if total_ram > 0.0 {
                    (used_ram / total_ram).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let cpus = sys.cpus();
                let cpu_usage_ratio = if !cpus.is_empty() {
                    let total_cpu: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
                    // Sysinfo cpu_usage is between 0.0 and 100.0 per core
                    (total_cpu / (cpus.len() as f32 * 100.0)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Envoie la mise à jour aux abonnés (le SensorySynthesisEngine).
                // Si l'Engine n'existe plus (Receiver droppé), le worker s'arrête de lui-même.
                if sender
                    .send(SystemMetrics {
                        ram_usage_ratio,
                        cpu_usage_ratio,
                    })
                    .is_err()
                {
                    tracing::debug!("HardwareMonitor stoppé (channel fermé).");
                    break;
                }

                // Poll toutes les 5 secondes
                std::thread::sleep(Duration::from_secs(5));
            }
        });

        Self { receiver }
    }

    /// Création d'un monitor factice pour les tests d'intégration (Simulation de surcharge matérielle).
    pub fn dummy(ram_usage_ratio: f32, cpu_usage_ratio: f32) -> Self {
        let (_, receiver) = watch::channel(SystemMetrics {
            ram_usage_ratio,
            cpu_usage_ratio,
        });
        Self { receiver }
    }
}
