use headless_chrome::Tab;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct NotebookApi {
    pub tab: Arc<Tab>,
}

impl Drop for NotebookApi {
    fn drop(&mut self) {
        info!("♻️ Auto-Cleanup: Destitution de l'API Notebook et fermeture du pont CDP...");
        r2d2_browser::SovereignBrowser::shutdown_windows_bridge();
    }
}

impl NotebookApi {
    pub fn new(tab: Arc<Tab>) -> Self {
        let api = Self { tab };
        api.inject_hud();
        api
    }

    /// Injecte un indicateur visuel discret signalant le contrôle autonome
    fn inject_hud(&self) {
        let hud_js = r#"
            (function() {
                try {
                    let style = document.createElement('style');
                    style.id = 'r2d2-neon-frame';
                    style.textContent = `
                        @keyframes pulse-neon {
                            0% { box-shadow: inset 0 0 5px #00f3ff, 0 0 5px #00f3ff; border-color: #00f3ff; }
                            50% { box-shadow: inset 0 0 20px #00f3ff, 0 0 20px #00f3ff; border-color: #00c3ff; }
                            100% { box-shadow: inset 0 0 5px #00f3ff, 0 0 5px #00f3ff; border-color: #00f3ff; }
                        }
                        body::after {
                            content: '';
                            position: fixed;
                            top: 0; left: 0; right: 0; bottom: 0;
                            border: 2px solid #00f3ff;
                            pointer-events: none;
                            z-index: 999999;
                            animation: pulse-neon 2s infinite ease-in-out;
                        }
                    `;
                    if(!document.getElementById('r2d2-neon-frame')) {
                        document.head.appendChild(style);
                    }
                } catch(e) {}
            })();
        "#;
        let _ = self.tab.evaluate(hud_js, false);
    }

    /// Exécute une requête RPC "batchexecute" directement dans le contexte de la page
    /// Extraction du reverse-engineering 'notebooklm-py' pour une souveraineté locale totale
    pub fn execute_rpc(
        &self,
        rpc_id: &str,
        path: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let params_str = serde_json::to_string(&params)?;

        let js = format!(
            r#"
            (async function() {{
                try {{
                    let csrfToken = window.WIZ_global_data?.SNlM0e || (document.body.innerHTML.match(/"SNlM0e":"([^"]+)"/) || [])[1];
                    let sessionId = window.WIZ_global_data?.FdrFJe || (document.body.innerHTML.match(/"FdrFJe":"([^"]+)"/) || [])[1];
                    if (!csrfToken) return JSON.stringify({{ error: "Missing CSRF Token (SNlM0e)" }});
                    
                    let rpcid = {:?};
                    let paramsJson = {:?};
                    let inner = [rpcid, paramsJson, null, "generic"];
                    let freq = JSON.stringify([[inner]]);
                    
                    let formData = new URLSearchParams();
                    formData.append('f.req', freq);
                    formData.append('at', csrfToken);
                    
                    let url = "/_/LabsTailwindUi/data/batchexecute?rpcids=" + rpcid + "&source-path=" + encodeURIComponent({:?}) + "&rt=c";
                    if (sessionId) url += "&f.sid=" + sessionId;
                    
                    let resp = await fetch(url, {{
                        method: 'POST',
                        headers: {{ 'Content-Type': 'application/x-www-form-urlencoded;charset=UTF-8' }},
                        body: formData.toString()
                    }});
                    
                    if (!resp.ok) return JSON.stringify({{ error: "HTTP " + resp.status }});
                    let text = await resp.text();
                    
                    // Nettoyage Anti-XSSI
                    let cleaned = text.startsWith(")]}}'") ? text.substring(text.indexOf("\n") + 1) : text;
                    
                    // Parsing du format Chunked Google
                    let chunks = cleaned.split("\n");
                    for (let i = 0; i < chunks.length; i++) {{
                        try {{
                            let chunk = JSON.parse(chunks[i]);
                            // Look for `wrb.fr` payload. In python format is [ ["wrb.fr", "CCqFvf", "[null, null, ..."] ]
                            // Google nests it sometimes inside arrays. We just crawl.
                            let found = null;
                            const crawl = (arr) => {{
                                if (!Array.isArray(arr)) return;
                                if (arr[0] === "wrb.fr" && arr[1] === rpcid) {{
                                    found = arr[2];
                                    return;
                                }}
                                for (let item of arr) crawl(item);
                            }};
                            crawl(chunk);
                            
                            if (found !== null) {{
                                let parsedData = null;
                                try {{ parsedData = JSON.parse(found); }} catch(e) {{ parsedData = found; }}
                                return JSON.stringify({{ data: parsedData }});
                            }}
                        }} catch(e) {{}}
                    }}
                    return JSON.stringify({{ error: "RPC Result Not Found in parsing" }});
                }} catch (e) {{
                    return JSON.stringify({{ error: e.toString() }});
                }}
            }})()
            "#,
            rpc_id, params_str, path
        );

        let remote_obj = self.tab.evaluate(&js, true)?;

        if let Some(val) = remote_obj.value {
            if let Some(json_str) = val.as_str() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(err) = parsed.get("error") {
                        if err.as_str() == Some("RPC Result Not Found in parsing") {
                            // Mutation RPC (comme LBwxtb) peut ne rien retourner de standard.
                            // On considère que c'est un succès si pas d'autre erreur.
                            return Ok(serde_json::json!({ "status": "success_no_payload" }));
                        }
                        error!("RPC Error ({}): {}", rpc_id, err);
                        return Err(anyhow::anyhow!("RPC Fetch Error: {}", err));
                    }
                    if let Some(data) = parsed.get("data") {
                        return Ok(data.clone());
                    }
                    return Ok(parsed);
                }
            }
            return Ok(val);
        }

        Err(anyhow::anyhow!(
            "Aucune réponse JSON valide du RPC {}",
            rpc_id
        ))
    }

    /// PURE RPC: Ask a question directly via batchexecute GenerateFreeFormStreamed instead of DOM injection
    pub fn chat_ask(&self, notebook_uuid: &str, prompt: &str) -> anyhow::Result<String> {
        info!("💬 Envoi RPC de la question vers NotebookLM...");
        let js = format!(
            r#"
            window.__r2d2_chat_result = null;
            window.__r2d2_chat_error = null;
            (async function() {{
                try {{
                    let csrfToken = window.WIZ_global_data?.SNlM0e || (document.body.innerHTML.match(/"SNlM0e":"([^"]+)"/) || [])[1];
                    let sessionId = window.WIZ_global_data?.FdrFJe || (document.body.innerHTML.match(/"FdrFJe":"([^"]+)"/) || [])[1];
                    if (!csrfToken) {{
                        window.__r2d2_chat_error = "Missing CSRF Token (SNlM0e)";
                        return;
                    }}
                    
                    let question = {:?};
                    let notebookId = {:?};
                    let conversationId = (crypto && crypto.randomUUID) ? crypto.randomUUID() : ""; 
                    
                    let params = [
                        [], 
                        question,
                        null,
                        [2, null, [1], [1]],
                        conversationId,
                        null,
                        null,
                        notebookId,
                        1
                    ];
                    
                    let paramsJson = JSON.stringify(params);
                    let freq = JSON.stringify([null, paramsJson]);
                    
                    let formData = new URLSearchParams();
                    formData.append('f.req', freq);
                    formData.append('at', csrfToken);
                    
                    let url = "/_/LabsTailwindUi/data/google.internal.labs.tailwind.orchestration.v1.LabsTailwindOrchestrationService/GenerateFreeFormStreamed?rt=c";
                    if (sessionId) url += "&f.sid=" + sessionId;
                    
                    let resp = await fetch(url, {{
                        method: 'POST',
                        headers: {{ 'Content-Type': 'application/x-www-form-urlencoded;charset=UTF-8' }},
                        body: formData.toString()
                    }});
                    
                    if (!resp.ok) {{
                        window.__r2d2_chat_error = "HTTP " + resp.status;
                        return;
                    }}
                    let text = await resp.text();
                    window.__r2d2_chat_result = text;
                }} catch (e) {{
                    window.__r2d2_chat_error = e.toString();
                }}
            }})()
            "#,
            prompt, notebook_uuid
        );

        // On n'attend pas la Propmise (false), le JS tourne en background dans Chrome.
        self.tab.evaluate(&js, false)?;

        // Boucle de polling courte pour récupérer le résultat sans Timeout CDP
        let mut final_data: Option<String> = None;
        let mut error_data: Option<String> = None;

        for i in 0..120 {
            // Max 120 secondes
            std::thread::sleep(std::time::Duration::from_secs(1));
            info!("⏳ RPC Polling - Vérification #{} ...", i + 1);

            if let Ok(res) = self.tab.evaluate("window.__r2d2_chat_error", false) {
                if let Some(val) = res.value.as_ref().and_then(|v| v.as_str()) {
                    error_data = Some(val.to_string());
                    break;
                }
            }
            if let Ok(res) = self.tab.evaluate("window.__r2d2_chat_result", false) {
                if let Some(val) = res.value.as_ref().and_then(|v| v.as_str()) {
                    final_data = Some(val.to_string());
                    break;
                }
            }
        }

        // Nettoyage de l'espace global
        let _ = self.tab.evaluate(
            "window.__r2d2_chat_result = null; window.__r2d2_chat_error = null;",
            false,
        );

        if let Some(err) = error_data {
            error!("RPC Chat Error: {}", err);
            return Err(anyhow::anyhow!("Chat Fetch Error: {}", err));
        }

        if let Some(data) = final_data {
            let parsed_answer = Self::parse_generate_freeform_stream(&data);
            if parsed_answer.is_empty() {
                return Err(anyhow::anyhow!(
                    "Réponse RPC interceptée mais vide de contenu"
                ));
            }
            return Ok(parsed_answer);
        }

        Err(anyhow::anyhow!(
            "Aucune réponse validable de l'API chat_ask (Timeout 120s)"
        ))
    }

    /// Extract the Markdown formatted answer from the chunked JSON stream returned by GenerateFreeFormStreamed
    fn parse_generate_freeform_stream(response_text: &str) -> String {
        let mut clean_text = response_text.trim();
        if clean_text.starts_with(")]}'") {
            clean_text = &clean_text[4..];
        }

        let mut best_marked_answer = String::new();
        let mut best_unmarked_answer = String::new();

        for line in clean_text.split('\n') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(arr) = data.as_array() {
                    for item in arr {
                        if let Some(item_arr) = item.as_array() {
                            if item_arr.len() < 3 {
                                continue;
                            }
                            if item_arr[0].as_str() != Some("wrb.fr") {
                                continue;
                            }

                            // Extract inner JSON from item[2]
                            if let Some(inner_json_str) = item_arr[2].as_str() {
                                if let Ok(inner_data) =
                                    serde_json::from_str::<serde_json::Value>(inner_json_str)
                                {
                                    if let Some(inner_arr) = inner_data.as_array() {
                                        if let Some(first) =
                                            inner_arr.first().and_then(|v| v.as_array())
                                        {
                                            if let Some(text) =
                                                first.first().and_then(|v| v.as_str())
                                            {
                                                // Check if it's the "marked" final answer
                                                let is_answer = first.len() > 4
                                                    && first[4].as_array().is_some_and(|a| {
                                                        a.last().and_then(|last| last.as_u64())
                                                            == Some(1)
                                                    });

                                                if is_answer
                                                    && text.len() > best_marked_answer.len()
                                                {
                                                    best_marked_answer = text.to_string();
                                                } else if !is_answer
                                                    && text.len() > best_unmarked_answer.len()
                                                {
                                                    best_unmarked_answer = text.to_string();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !best_marked_answer.is_empty() {
            best_marked_answer
        } else {
            best_unmarked_answer
        }
    }

    /// Extrait le rapport d'une réponse de POLL_RESEARCH
    fn extract_poll_report(
        res: &serde_json::Value,
        target_task_id: &str,
    ) -> Option<(String, String)> {
        if let Some(arr) = res.as_array() {
            // Task has format: ["u... task ID", [... task info]]
            if arr.len() >= 2 {
                if let Some(id) = arr[0].as_str() {
                    if id == target_task_id {
                        if let Some(t_info) = arr[1].as_array() {
                            let status = t_info.get(4).and_then(|v| v.as_u64()).unwrap_or(0);
                            if status == 2 || status == 6 {
                                // terminée! extraction:
                                let mut title = "Web Source".to_string();
                                let mut report = "".to_string();

                                if let Some(sources) = t_info
                                    .get(3)
                                    .and_then(|v| v.as_array())
                                    .and_then(|v| v.first())
                                    .and_then(|v| v.as_array())
                                {
                                    for src_val in sources {
                                        if let Some(src) = src_val.as_array() {
                                            if let Some(titles_arr) =
                                                src.get(1).and_then(|v| v.as_array())
                                            {
                                                if titles_arr.len() >= 2 {
                                                    title = titles_arr[0]
                                                        .as_str()
                                                        .unwrap_or("Source")
                                                        .to_string();
                                                    report = titles_arr[1]
                                                        .as_str()
                                                        .unwrap_or("")
                                                        .to_string();
                                                }
                                            } else if let Some(t) =
                                                src.get(1).and_then(|v| v.as_str())
                                            {
                                                title = t.to_string();
                                            }
                                        }
                                    }
                                }
                                return Some((title, report));
                            } else {
                                return None; // en cours
                            }
                        }
                    }
                }
            }
            // Recherche récursive
            for val in arr {
                if let Some(found) = Self::extract_poll_report(val, target_task_id) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Extrait TOUTES les tâches Deep Search de la réponse de polling (e3bVqc)
    fn extract_all_poll_tasks(
        res: &serde_json::Value,
        tasks: &mut Vec<(String, u64, String, String)>,
    ) {
        if let Some(arr) = res.as_array() {
            // Task has format: ["u... task ID", [... task info]]
            if arr.len() >= 2 {
                if let Some(id) = arr[0].as_str() {
                    // It looks like a UUID-based task ID
                    if id.len() > 20 && id.contains("-") {
                        if let Some(t_info) = arr[1].as_array() {
                            let status = t_info.get(4).and_then(|v| v.as_u64()).unwrap_or(0);
                            let mut title = "Web Source".to_string();
                            let mut report = "".to_string();

                            if status == 2 || status == 6 {
                                if let Some(sources) = t_info
                                    .get(3)
                                    .and_then(|v| v.as_array())
                                    .and_then(|v| v.first())
                                    .and_then(|v| v.as_array())
                                {
                                    for src_val in sources {
                                        if let Some(src) = src_val.as_array() {
                                            if let Some(titles_arr) =
                                                src.get(1).and_then(|v| v.as_array())
                                            {
                                                if titles_arr.len() >= 2 {
                                                    title = titles_arr[0]
                                                        .as_str()
                                                        .unwrap_or("Source")
                                                        .to_string();
                                                    report = titles_arr[1]
                                                        .as_str()
                                                        .unwrap_or("")
                                                        .to_string();
                                                }
                                            } else if let Some(t) =
                                                src.get(1).and_then(|v| v.as_str())
                                            {
                                                title = t.to_string();
                                            }
                                        }
                                    }
                                }
                            }
                            // Store everything, even if status is not 2, to track running tasks
                            tasks.push((id.to_string(), status, title, report));
                        }
                    }
                }
            }
            // Recherche récursive
            for val in arr {
                Self::extract_all_poll_tasks(val, tasks);
            }
        }
    }

    /// Verifie si le Notebook Actuel est authorisé en écriture via la balise [R2D2]
    pub fn verify_r2d2_access(&self) -> anyhow::Result<()> {
        // Le check [R2D2] est désactivé pour permettre la consultation (lecture)
        // d'experts non marqués comme souverains (ex: RustyMaster).
        Ok(())
    }

    /// Crée un nouveau Notebook Vierge en RPC
    pub fn create_notebook(&self, name: &str) -> anyhow::Result<String> {
        let r2d2_name = format!("[R2D2] {}", name);
        info!("Fabrication RPC du Carnet Expert: {}", r2d2_name);

        self.tab.navigate_to("https://notebooklm.google.com/")?;
        self.tab.wait_until_navigated()?;

        // Params = [title, null, null, [2], [1]]
        let params = serde_json::json!([r2d2_name, null, null, [2], [1]]);
        let response = self.execute_rpc("CCqFvf", "/", params)?;

        // Le UUID est typiquement à l'index 2 (ex: [titre, null, "UUID", ...])
        let mut uuid = String::new();
        if let Some(arr) = response.as_array() {
            if arr.len() > 2 {
                if let Some(u) = arr[2].as_str() {
                    uuid = u.to_string();
                }
            } else if !arr.is_empty() {
                // Fallback (des fois il parse différemment)
                if let Some(item_arr) = arr[0].as_array() {
                    if item_arr.len() > 2 {
                        if let Some(u) = item_arr[2].as_str() {
                            uuid = u.to_string();
                        }
                    }
                }
            }
        }

        if uuid.is_empty() {
            return Err(anyhow::anyhow!(
                "Format de réponse inattendu lors de la création CCqFvf: {:?}",
                response
            ));
        }

        info!("✅ Expert '{}' forgé ! UUID: {}", r2d2_name, uuid);

        // On navigue sur ce carnet. NotebookApi s'occupe de garder le focus.
        let url = format!("https://notebooklm.google.com/notebook/{}", uuid);
        self.tab.navigate_to(&url)?;
        self.tab.wait_until_navigated()?;

        Ok(url)
    }

    /// Ouvre la modale RPC "Deep Research" asynchrone et importe sa trouvaille
    pub fn add_deep_search_source(&self, notebook_uuid: &str, query: &str) -> anyhow::Result<()> {
        let path = format!("/notebook/{}", notebook_uuid);

        // 1. Démarrer Deep Research
        info!("🔎 Injection Deep Search RPC : {}", query);
        let start_params = serde_json::json!([null, [1], [query, 1], 5, notebook_uuid]);
        let start_res = self.execute_rpc("QA9ei", &path, start_params)?;

        let mut task_id = "";
        if let Some(arr) = start_res.as_array() {
            if let Some(t_str) = arr.first().and_then(|v| v.as_str()) {
                task_id = t_str;
            } else if let Some(sub_arr) = arr.first().and_then(|v| v.as_array()) {
                task_id = sub_arr.first().and_then(|v| v.as_str()).unwrap_or("");
            }
        }

        if task_id.is_empty() {
            return Err(anyhow::anyhow!(
                "Impossible d'extraire la TaskID de Deep Search RPC"
            ));
        }
        info!("⏳ Deep Research Task ID: {}. Polling engagé...", task_id);

        // 2. Polling (e3bVqc)
        let poll_params = serde_json::json!([null, null, notebook_uuid]);
        let final_title;
        let final_report;

        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed().as_secs() > 180 {
                return Err(anyhow::anyhow!(
                    "Timeout: la recherche Deep Research dépasse les 3 minutes."
                ));
            }

            std::thread::sleep(std::time::Duration::from_secs(5));
            let poll_res = self.execute_rpc("e3bVqc", &path, poll_params.clone());

            if let Ok(res) = poll_res {
                if let Some((title, report)) = Self::extract_poll_report(&res, task_id) {
                    final_title = title;
                    final_report = report;
                    break;
                } else {
                    info!("... travail en cours par l'agent de recherche...");
                }
            }
        }

        if final_report.is_empty() {
            warn!("⚠️ Le rapport Deep Search est vide. L'import peut échouer.");
        }

        // 3. Importation du Rapport Source (LBwxtb)
        // Construction de l'entrée Source spéciale pour Web/Report
        let source_entry = serde_json::json!([
            null,
            [final_title, final_report],
            null,
            3,
            null,
            null,
            null,
            null,
            null,
            null,
            3
        ]);

        info!("📥 Importation de la ressource finalisée vers le Carnet...");
        let import_params = serde_json::json!([null, [1], task_id, notebook_uuid, [source_entry]]);
        let _ = self.execute_rpc("LBwxtb", &path, import_params)?;

        info!("✅ Auto-Validation. Source assimilée purement RPC.");
        Ok(())
    }

    /// Daemon Watcher : Importe automatiquement les recherches Deep Search en attente/en cours
    pub fn auto_import_pending_searches(&self, notebook_uuid: &str) -> anyhow::Result<()> {
        let path = format!("/notebook/{}", notebook_uuid);
        let poll_params = serde_json::json!([null, null, notebook_uuid]);
        let mut processed = std::collections::HashSet::new();

        info!(
            "👁️ Démarrage du Watcher Deep Search pour le carnet {}...",
            notebook_uuid
        );

        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed().as_secs() > 3600 {
                return Err(anyhow::anyhow!("Timeout global: arrêt du watcher (1h)"));
            }

            let poll_res = self.execute_rpc("e3bVqc", &path, poll_params.clone())?;
            let mut tasks = Vec::new();
            Self::extract_all_poll_tasks(&poll_res, &mut tasks);

            let mut running = 0;
            for (id, status, title, report) in tasks {
                if processed.contains(&id) {
                    continue; // Déjà traité dans cette boucle
                }

                // 1 ou 5 = Running
                if status == 1 || status == 5 {
                    running += 1;
                    continue;
                }

                // De base, on ne veut pas importer des requêtes historiques de l'usager qu'il a déjà importées avant.
                // Donc on ne déclenchera l'import QUE si la tâche PASSE de l'état (running) à l'état (done) pendant qu'on regarde,
                // ou bien si elle vient d'être terminée récemment (pour palier aux requêtes manuelles, on maintiendra un fichier /tmp).
                // Simplification : le script trace les "running" vus, et import ce qu'il a vu !
                if status == 2 || status == 6 {
                    let cache_file = "/tmp/r2d2_imported_tasks.txt";
                    let history = std::fs::read_to_string(cache_file).unwrap_or_default();
                    if !history.contains(&id) {
                        info!("📥 Importation de la tâche terminée id: {}", id);
                        let source_entry = serde_json::json!([
                            null,
                            [title, report],
                            null,
                            3,
                            null,
                            null,
                            null,
                            null,
                            null,
                            null,
                            3
                        ]);
                        let import_params =
                            serde_json::json!([null, [1], &id, notebook_uuid, [source_entry]]);
                        if let Err(e) = self.execute_rpc("LBwxtb", &path, import_params) {
                            warn!("⚠️ Erreur lors de l'importation de {}: {}", id, e);
                        } else {
                            info!("✅ Importation réussie : {}", title);
                            // Enregistrer dans le cache
                            use std::io::Write;
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(cache_file)
                            {
                                let _ = writeln!(f, "{}", id);
                            }
                        }
                    }
                    processed.insert(id);
                }
            }

            if running == 0 {
                info!("🏁 Plus aucune recherche en cours. Fermeture du Watcher.");
                break;
            } else {
                info!("⏳ {} recherche(s) en cours, attente 5s...", running);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }

        Ok(())
    }

    /// Liste tous les carnets via RPC
    pub fn list_notebooks(&self) -> anyhow::Result<Vec<(String, String)>> {
        // payload: [null, 1, null, [2]]
        let params = serde_json::json!([null, 1, null, [2]]);
        let response = self.execute_rpc("wXbhsf", "/", params)?;

        let mut notebooks = Vec::new();
        // Let's recursively search for arrays that look like [Title_str, ..., UUID_str]
        fn extract_nbs(val: &serde_json::Value, list: &mut Vec<(String, String)>) {
            if let Some(arr) = val.as_array() {
                if arr.len() > 2 {
                    if let (Some(title), Some(id)) = (arr[0].as_str(), arr[2].as_str()) {
                        // Check if id looks like a valid UUID (not wrb.fr etc)
                        if id != "wrb.fr" && id.len() > 10 && id.contains("-") {
                            let mut t = title.to_string();
                            // Fix Google UI convention where empty title implies "Untitled notebook"
                            if t.is_empty() {
                                t = "Untitled notebook".to_string();
                            }
                            list.push((id.to_string(), t));
                        }
                    }
                }
                for item in arr {
                    extract_nbs(item, list);
                }
            }
        }

        extract_nbs(&response, &mut notebooks);
        Ok(notebooks)
    }

    /// Supprime un carnet via RPC
    pub fn delete_notebook(&self, notebook_uuid: &str) -> anyhow::Result<()> {
        info!("🗑️ Suppression RPC du carnet : {}", notebook_uuid);
        let params = serde_json::json!([[notebook_uuid], [2]]);
        let _ = self.execute_rpc("WWINqb", "/", params)?;
        Ok(())
    }

    /// Purge tous les carnets "Untitled notebook"
    pub fn purge_untitled_notebooks(&self) -> anyhow::Result<usize> {
        info!("🧹 Lancement du protocole de nettoyage...");
        self.tab.navigate_to("https://notebooklm.google.com/")?;
        self.tab.wait_until_navigated()?;

        let nbs = self.list_notebooks()?;
        let mut deleted_count = 0;

        for (id, title) in nbs {
            let t = title.to_lowercase();
            if t.contains("untitled notebook")
                || t.contains("carnet sans titre")
                || t.contains("notebook sans titre")
                || t.contains("carnet sans nom")
            {
                if let Err(e) = self.delete_notebook(&id) {
                    error!("Erreur suppression {}: {}", id, e);
                } else {
                    info!("✅ Carnet '{}' ({}) incinéré !", title, id);
                    deleted_count += 1;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }

        info!(
            "🧹 Cycle de nettoyage terminé. {} scories effacées.",
            deleted_count
        );
        Ok(deleted_count)
    }

    /// Rescue UI method : Clic automatique sur "+ Importer" pour valider un Deep Search
    pub fn click_import_button(&self) -> anyhow::Result<()> {
        let js = r#"
            (function() {
                let btns = Array.from(document.querySelectorAll('button, [role="button"]'));
                let importBtn = btns.find(b => {
                    let txt = (b.innerText || "").toLowerCase();
                    return txt.includes("importer") || txt.includes("import ");
                });
                if (importBtn && !importBtn.disabled) {
                    importBtn.click();
                    return true;
                }
                return false;
            })()
        "#;

        for _ in 0..10 {
            if let Ok(eval) = self.tab.evaluate(js, false) {
                if let Some(serde_json::Value::Bool(true)) = eval.value {
                    info!("✅ Bouton d'importation graphique activé !");
                    return Ok(());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        Err(anyhow::anyhow!(
            "Impossible de trouver le bouton '+ Importer'"
        ))
    }
}
