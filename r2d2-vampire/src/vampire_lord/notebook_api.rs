use chromiumoxide::Page;
use std::sync::Arc;
use tracing::{error, info};

pub struct NotebookApi {
    pub tab: Arc<Page>,
}

impl NotebookApi {
    pub async fn new(tab: Arc<Page>) -> Self {
        let api = Self { tab };
        api.inject_hud().await;
        api
    }

    /// Injecte un indicateur visuel discret signalant le contrôle autonome
    async fn inject_hud(&self) {
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
        let _ = self.tab.evaluate(hud_js).await;
    }

    /// Exécute une requête RPC "batchexecute" Asynchrone Promise-Based
    pub async fn execute_rpc(
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
                    
                    let cleaned = text.startsWith(")]}}'") ? text.substring(text.indexOf("\n") + 1) : text;
                    let chunks = cleaned.split("\n");
                    for (let i = 0; i < chunks.length; i++) {{
                        try {{
                            let chunk = JSON.parse(chunks[i]);
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

        let remote_obj = self.tab.evaluate(js.as_str()).await?;
        if let Some(val) = remote_obj.value() {
            if let Some(json_str) = val.as_str() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(err) = parsed.get("error") {
                        if err.as_str() == Some("RPC Result Not Found in parsing") {
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
            return Ok(val.clone());
        }

        Err(anyhow::anyhow!(
            "Aucune réponse JSON valide du RPC {}",
            rpc_id
        ))
    }

    /// PURE RPC: Async Promise-based evaluation with native 120s Tokio Timeout.
    /// Éradication complète du Polling de Scraping DOM de l'ancienne version.
    pub async fn chat_ask(&self, notebook_uuid: &str, prompt: &str) -> anyhow::Result<String> {
        info!("💬 Envoi RPC de la question vers NotebookLM (Mode Promise Async natif CDP)...");
        let js = format!(
            r#"
            (async function() {{
                try {{
                    let csrfToken = window.WIZ_global_data?.SNlM0e || (document.body.innerHTML.match(/"SNlM0e":"([^"]+)"/) || [])[1];
                    let sessionId = window.WIZ_global_data?.FdrFJe || (document.body.innerHTML.match(/"FdrFJe":"([^"]+)"/) || [])[1];
                    if (!csrfToken) return JSON.stringify({{ error: "Missing CSRF Token (SNlM0e)" }});
                    
                    let question = {:?};
                    let notebookId = {:?};
                    let conversationId = (crypto && crypto.randomUUID) ? crypto.randomUUID() : ""; 
                    
                    let params = [
                        [], question, null, [2, null, [1], [1]], conversationId, null, null, notebookId, 1
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
                    
                    if (!resp.ok) return JSON.stringify({{ error: "HTTP " + resp.status }});
                    
                    let reader = resp.body.getReader();
                    let decoder = new TextDecoder("utf-8");
                    let accumulated = "";
                    let final_buffer = null;
                    
                    while (true) {{
                        let {{done, value}} = await reader.read();
                        if (value) accumulated += decoder.decode(value, {{stream: !done}});
                        
                        let chunks = accumulated.split('\n');
                        let last_line = chunks.pop() || "";
                        
                        for (let line of chunks) {{
                            line = line.trim();
                            if (!line) continue;
                            if (line.startsWith(")]}}'")) line = line.substring(4);
                            try {{
                                let data = JSON.parse(line);
                                if (!Array.isArray(data)) continue;
                                for(let item of data) {{
                                    if(Array.isArray(item) && item.length >= 3 && item[0] === "wrb.fr") {{
                                        let inner = JSON.parse(item[2]);
                                        if (Array.isArray(inner) && Array.isArray(inner[0]) && typeof inner[0][0] === "string") {{
                                            final_buffer = inner[0][0];
                                        }}
                                    }}
                                }}
                            }} catch(e) {{ }}
                        }}
                        accumulated = last_line;
                        
                        if (done) {{
                            if (final_buffer) return JSON.stringify({{ data: final_buffer }});
                            return JSON.stringify({{ error: "Stream closed without data" }});
                        }}
                    }}
                }} catch (e) {{
                    return JSON.stringify({{ error: e.toString() }});
                }}
            }})();
            "#,
            prompt, notebook_uuid
        );

        // Appel CDP Natif avec garantie Tokio (Attente réelle sur le bloc JS `await reader.read()`)
        let timeout_future = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            self.tab.evaluate(js.as_str()),
        );

        match timeout_future.await {
            Ok(Ok(remote_obj)) => {
                if let Some(val) = remote_obj.value().and_then(|v| v.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(val) {
                        if let Some(err) = parsed.get("error") {
                            return Err(anyhow::anyhow!("RPC Chat Error: {}", err));
                        }
                        if let Some(data) = parsed.get("data").and_then(|d| d.as_str()) {
                            return Ok(data.to_string());
                        }
                    }
                    return Err(anyhow::anyhow!("Erreur parse JSON: {}", val));
                }
                Err(anyhow::anyhow!("Réponse inattendue de l'évaluateur JS"))
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("CDP Evaluation Crash: {}", e)),
            Err(_) => Err(anyhow::anyhow!(
                "Timeout critique (120s) sur la Promise de Chat."
            )),
        }
    }

    /// Extract the Markdown formatted answer from the chunked JSON stream
    fn extract_poll_report(
        res: &serde_json::Value,
        target_task_id: &str,
    ) -> Option<(String, String)> {
        if let Some(arr) = res.as_array() {
            if arr.len() >= 2 {
                if let Some(id) = arr[0].as_str() {
                    if id == target_task_id {
                        if let Some(t_info) = arr[1].as_array() {
                            let status = t_info.get(4).and_then(|v| v.as_u64()).unwrap_or(0);
                            if status == 2 || status == 6 {
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
                                return None;
                            }
                        }
                    }
                }
            }
            for val in arr {
                if let Some(found) = Self::extract_poll_report(val, target_task_id) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn extract_all_poll_tasks(
        res: &serde_json::Value,
        tasks: &mut Vec<(String, u64, String, String)>,
    ) {
        if let Some(arr) = res.as_array() {
            if arr.len() >= 2 {
                if let Some(id) = arr[0].as_str() {
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
                                            if let Some(t) = src.get(1).and_then(|v| v.as_array()) {
                                                if t.len() >= 2 {
                                                    title = t[0]
                                                        .as_str()
                                                        .unwrap_or("Source")
                                                        .to_string();
                                                    report =
                                                        t[1].as_str().unwrap_or("").to_string();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            tasks.push((id.to_string(), status, title, report));
                        }
                    }
                }
            }
            for val in arr {
                Self::extract_all_poll_tasks(val, tasks);
            }
        }
    }

    pub fn verify_r2d2_access(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn create_notebook(&self, name: &str) -> anyhow::Result<String> {
        let r2d2_name = format!("[R2D2] {}", name);
        info!("Fabrication RPC du Carnet Expert: {}", r2d2_name);

        self.tab.goto("https://notebooklm.google.com/").await?;
        self.tab.wait_for_navigation().await?;

        let params = serde_json::json!([r2d2_name, null, null, [2], [1]]);
        let response = self.execute_rpc("CCqFvf", "/", params).await?;

        let mut uuid = String::new();
        if let Some(arr) = response.as_array() {
            if arr.len() > 2 {
                if let Some(u) = arr[2].as_str() {
                    uuid = u.to_string();
                }
            } else if !arr.is_empty() {
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
            return Err(anyhow::anyhow!("Format RPC CCqFvf inattendu"));
        }

        let url = format!("https://notebooklm.google.com/notebook/{}", uuid);
        self.tab.goto(&url).await?;
        self.tab.wait_for_navigation().await?;
        Ok(url)
    }

    pub async fn add_deep_search_source(
        &self,
        notebook_uuid: &str,
        query: &str,
    ) -> anyhow::Result<()> {
        let path = format!("/notebook/{}", notebook_uuid);
        let start_params = serde_json::json!([null, [1], [query, 1], 5, notebook_uuid]);
        let start_res = self.execute_rpc("QA9ei", &path, start_params).await?;

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

        let poll_params = serde_json::json!([null, null, notebook_uuid]);
        let final_title;
        let final_report;

        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed().as_secs() > 180 {
                return Err(anyhow::anyhow!("Timeout Deep Research (3 min)."));
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Ok(res) = self.execute_rpc("e3bVqc", &path, poll_params.clone()).await {
                if let Some((title, report)) = Self::extract_poll_report(&res, task_id) {
                    final_title = title;
                    final_report = report;
                    break;
                }
            }
        }

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
        let import_params = serde_json::json!([null, [1], task_id, notebook_uuid, [source_entry]]);
        self.execute_rpc("LBwxtb", &path, import_params).await?;
        Ok(())
    }

    pub async fn auto_import_pending_searches(&self, notebook_uuid: &str) -> anyhow::Result<()> {
        let path = format!("/notebook/{}", notebook_uuid);
        let poll_params = serde_json::json!([null, null, notebook_uuid]);
        let mut processed = std::collections::HashSet::new();

        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed().as_secs() > 3600 {
                return Ok(());
            }
            let poll_res = self
                .execute_rpc("e3bVqc", &path, poll_params.clone())
                .await?;
            let mut tasks = Vec::new();
            Self::extract_all_poll_tasks(&poll_res, &mut tasks);

            let mut running = 0;
            for (id, status, title, report) in tasks {
                if processed.contains(&id) {
                    continue;
                }
                if status == 1 || status == 5 {
                    running += 1;
                    continue;
                }
                if status == 2 || status == 6 {
                    let cache_file = "/tmp/r2d2_imported_tasks.txt";
                    let history = std::fs::read_to_string(cache_file).unwrap_or_default();
                    if !history.contains(&id) {
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
                        if self
                            .execute_rpc("LBwxtb", &path, import_params)
                            .await
                            .is_ok()
                        {
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
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
        Ok(())
    }

    pub async fn list_notebooks(&self) -> anyhow::Result<Vec<(String, String)>> {
        let params = serde_json::json!([null, 1, null, [2]]);
        let response = self.execute_rpc("wXbhsf", "/", params).await?;

        let mut notebooks = Vec::new();
        fn extract_nbs(val: &serde_json::Value, list: &mut Vec<(String, String)>) {
            if let Some(arr) = val.as_array() {
                if arr.len() > 2 {
                    if let (Some(title), Some(id)) = (arr[0].as_str(), arr[2].as_str()) {
                        if id != "wrb.fr" && id.len() > 10 && id.contains("-") {
                            let mut t = title.to_string();
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

    pub async fn delete_notebook(&self, notebook_uuid: &str) -> anyhow::Result<()> {
        let params = serde_json::json!([[notebook_uuid], [2]]);
        self.execute_rpc("WWINqb", "/", params).await?;
        Ok(())
    }

    pub async fn purge_untitled_notebooks(&self) -> anyhow::Result<usize> {
        self.tab.goto("https://notebooklm.google.com/").await?;
        self.tab.wait_for_navigation().await?;
        let nbs = self.list_notebooks().await?;
        let mut deleted_count = 0;
        for (id, title) in nbs {
            let t = title.to_lowercase();
            if (t.contains("untitled") || t.contains("sans titre") || t.contains("sans nom"))
                && self.delete_notebook(&id).await.is_ok()
            {
                deleted_count += 1;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
        Ok(deleted_count)
    }

    pub async fn click_import_button(&self) -> anyhow::Result<()> {
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
            if let Ok(eval) = self.tab.evaluate(js).await {
                if let Some(serde_json::Value::Bool(true)) = eval.value() {
                    return Ok(());
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Err(anyhow::anyhow!(
            "Impossible de trouver le bouton '+ Importer'"
        ))
    }
}
