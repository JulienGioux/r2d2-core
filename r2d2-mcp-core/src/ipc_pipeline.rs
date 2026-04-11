use crate::mcp_types::{IpcMcpRequest, IpcMcpResponse};
use rkyv::check_archived_root;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info};

pub async fn start_unix_socket(server: Arc<crate::SuperMcpServer>, socket_path: &str) {
    let _ = std::fs::remove_file(socket_path); // Nettoyage agressif d'un vieil artefact
    let listener =
        UnixListener::bind(socket_path).expect("Echec critique du bind Unix Socket (IPC)");
    info!(
        "🚀 Neural-Link IPC Actif. [Zero-Copy Unix Socket: {}]",
        socket_path
    );

    while let Ok((mut socket, _)) = listener.accept().await {
        let srv = Arc::clone(&server);
        tokio::spawn(async move {
            debug!("Nouvelle connexion physique détectée sur le socket IPC.");
            if let Err(e) = handle_ipc_client(&mut socket, srv).await {
                error!("Erreur de liaison Client IPC: {}", e);
            }
            debug!("Connexion IPC terminée.");
        });
    }
}

async fn handle_ipc_client(
    socket: &mut UnixStream,
    server: Arc<crate::SuperMcpServer>,
) -> anyhow::Result<()> {
    const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024; // [Anti-DoS] Limite dure à 10 Mo
    let mut payload_buf = Vec::with_capacity(8 * 1024); // [Zero-Alloc] Réutilisation d'une seule allocation mémoire (Fix #30)

    loop {
        let mut len_buf = [0u8; 4];
        // Si la lecture échoue, le client a fermé la connexion, on débranche doucement.
        if socket.read_exact(&mut len_buf).await.is_err() {
            return Ok(());
        }

        let length = u32::from_le_bytes(len_buf) as usize;

        // --- 🔒 [SECURITY GUARD] OOM Prevention ---
        if length > MAX_PAYLOAD_SIZE {
            error!(
                "[SECURITY] Payload IPC massive rejetée (Anti-DoS). Longueur: {}",
                length
            );
            return Ok(()); // Coupe l'attaque immédiatement
        }

        // --- 🚀 [ZERO-ALLOCATION] Mutate existing Capacity ---
        payload_buf.resize(length, 0u8);
        socket.read_exact(&mut payload_buf).await?;

        // 1. Zéro-Copie : Validation stricte et overloay immuable
        // --- ⚡ [ANTI-STARVATION & ZERO-ALLOCATION PORT] ---
        // Exécution bloquante sur un thread Tokio coopératif pour protéger l'Event Loop
        // sans obliger le payload_buf à avoir le trait 'static (Pas de clone mémoire).
        let (method, params_opt) = tokio::task::block_in_place(|| {
            let archived = match check_archived_root::<IpcMcpRequest>(&payload_buf) {
                Ok(arch) => arch,
                Err(e) => {
                    error!("Trame binaire rkyv corrompue rejetée: {}", e);
                    return Err(());
                }
            };

            let m = archived.method.as_str().to_string(); // On clone la string (Minuscule ~10 bytes)
            let params_raw_slice = archived.params_raw.as_slice(); // ZCO
            let p: Option<serde_json::Value> = serde_json::from_slice(params_raw_slice).ok();

            Ok((m, p))
        })
        .unwrap_or_else(|_| ("".to_string(), None));

        if method.is_empty() {
            continue;
        }

        let mut response = IpcMcpResponse {
            success: false,
            result_raw: vec![],
        };

        // 2. Routage Hexagonal Pur (Domain Adapter)
        match server.execute_domain(&method, params_opt).await {
            Ok(res_json) => {
                response.success = true;
                response.result_raw = serde_json::to_vec(&res_json).unwrap_or_default();
            }
            Err((_code, err_msg)) => {
                response.success = false;
                response.result_raw = err_msg.into_bytes();
            }
        }

        // 3. Sérialisation Zero-Copy en sortie
        let bytes = match tokio::task::block_in_place(|| rkyv::to_bytes::<_, 256>(&response)) {
            Ok(b) => b,
            Err(e) => {
                error!("Erreur de sérialisation interne rkyv : {:?}", e);
                continue; // Skip silently to keep IPC engine alive
            }
        };

        let res_len = (bytes.len() as u32).to_le_bytes();
        socket.write_all(&res_len).await?;
        socket.write_all(&bytes).await?;
    }
}
