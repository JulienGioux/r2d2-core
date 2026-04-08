import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import puppeteer from "puppeteer-core";
import fs from "fs";
import path from "path";
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const DB_PATH = path.join(__dirname, "consultants.json");

// Initialiser la base si inexistante
if (!fs.existsSync(DB_PATH)) {
    fs.writeFileSync(DB_PATH, JSON.stringify({
        "RustyMaster": { url: "https://notebooklm.google.com/notebook/4dd65131-ea87-47a3-8958-a647351c4050", enabled: true },
        "Cuda": { url: "https://notebooklm.google.com/notebook/4f4488d7-7882-452c-9b49-d904f5e96508", enabled: true }
    }, null, 2));
}

function loadConsultants() {
    try {
        const raw = JSON.parse(fs.readFileSync(DB_PATH, "utf-8"));
        // Migration transparente pour les anciens formats { "Nom": "URL" }
        let needsSave = false;
        for (const key of Object.keys(raw)) {
            if (typeof raw[key] === "string") {
                raw[key] = { url: raw[key], enabled: true };
                needsSave = true;
            }
        }
        if (needsSave) saveConsultants(raw);
        return raw;
    } catch {
        return {};
    }
}

function saveConsultants(data) {
    fs.writeFileSync(DB_PATH, JSON.stringify(data, null, 2));
}

const server = new Server(
    { name: "SovereignConsultantBridge", version: "2.0.0" },
    { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
    const consultants = loadConsultants();
    const tools = [
        {
            name: "list_consultants",
            description: "Affiche tous les consultants NotebookLM disponibles, leur statut (activé/désactivé) et leurs URLs.",
            inputSchema: { type: "object", properties: {} }
        },
        {
            name: "add_consultant",
            description: "Ajoute un nouveau consultant NotebookLM à la configuration du serveur (sera autocomplétable ensuite).",
            inputSchema: {
                type: "object",
                properties: {
                    name: { type: "string" },
                    url: { type: "string", description: "L'URL complète du proxy NotebookLM" }
                },
                required: ["name", "url"]
            }
        },
        {
            name: "remove_consultant",
            description: "Supprime définitivement un consultant et ferme son onglet Chrome s'il était actif.",
            inputSchema: {
                type: "object",
                properties: {
                    name: { type: "string" }
                },
                required: ["name"]
            }
        },
        {
            name: "toggle_consultant",
            description: "Active ou désactive un consultant sans le supprimer (utile pour l'autocomplétion MCP).",
            inputSchema: {
                type: "object",
                properties: {
                    name: { type: "string" },
                    enabled: { type: "boolean" }
                },
                required: ["name", "enabled"]
            }
        }
    ];

    // Outils dynamiques par expert `ask_NAME`
    for (const [name, data] of Object.entries(consultants)) {
        if (data.enabled) {
            tools.push({
                name: `ask_${name.toLowerCase()}`,
                description: `Consulte l'expert '${name}' via son onglet connecté Stateful NotebookLM.`,
                inputSchema: {
                    type: "object",
                    properties: {
                        prompt: { type: "string", description: `Le contexte/la question posée à ${name}` }
                    },
                    required: ["prompt"]
                }
            });
        }
    }

    return { tools };
});

const activePages = new Map();
let browserInstance = null;

async function getBrowser() {
    if (browserInstance) {
        try {
            await browserInstance.version(); // Check de survie (si le Chrome Windows est fermé)
            return browserInstance;
        } catch {
            browserInstance = null;
        }
    }
    try {
        browserInstance = await puppeteer.connect({ browserURL: 'http://127.0.0.1:9222', defaultViewport: null });
        return browserInstance;
    } catch (e) {
        throw new Error("Impossible de se lier au CDP Chrome Windows. Vérifiez que Chrome est ouvert en mode Debug (--remote-debugging-port=9222).");
    }
}

// Extraction DOM NotebookLM Stateful (Onglets Persistants)
export async function queryNotebookLMStateful(name, url, prompt) {
    const browser = await getBrowser();
    let page = activePages.get(name);
    let isNewPage = false;
    
    // Self-Healing
    if (page) {
        try {
            await page.title(); 
        } catch {
            page = null;
            activePages.delete(name);
        }
    }

    if (!page) {
        page = await browser.newPage();
        await page.goto(url, { waitUntil: 'load' });
        activePages.set(name, page);
        isNewPage = true;
    }
    
    try {
        if (!isNewPage) {
            await page.bringToFront(); // Focus l'onglet existant
        }
        
        const textBefore = await page.evaluate(() => document.body.innerText);
        
        await page.waitForSelector('textarea', { timeout: 10000 });
        
        // Ciblage intelligent du vrai champ de Chat (contourne le panneau "Ajouter des sources" apparu sur les Stateful tabs)
        const chatInputHandle = await page.evaluateHandle(() => {
            const tas = Array.from(document.querySelectorAll('textarea'));
            let best = null;
            let maxY = -1;
            for (const t of tas) {
                const rect = t.getBoundingClientRect();
                if (rect.width > 0 && rect.height > 0) { // Uniquement les champs visibles
                    const p = (t.placeholder || t.getAttribute('aria-label') || '').toLowerCase();
                    // Signature du chat NotebookLM
                    if (p.includes('message') || p.includes('question') || p.includes('ask') || p.includes('répon') || p.includes('chat')) {
                        return t;
                    }
                    // Position par défaut = le bloc le plus en bas de la page
                    if (rect.y > maxY) {
                        maxY = rect.y;
                        best = t;
                    }
                }
            }
            return best || tas[tas.length - 1];
        });

        const chatEl = chatInputHandle.asElement();
        if (chatEl) {
            await chatEl.focus();
            await page.evaluate((el) => { el.value = ''; }, chatEl); // Purge de sécurité
            
            const lines = prompt.split('\\n');
            for (let i = 0; i < lines.length; i++) {
                await chatEl.type(lines[i]);
                if (i < lines.length - 1) {
                    await page.keyboard.down('Shift');
                    await page.keyboard.press('Enter');
                    await page.keyboard.up('Shift');
                }
            }
        } else {
            // Fallback aveugle absolu
            const lines = prompt.split('\\n');
            for (let i = 0; i < lines.length; i++) {
                await page.keyboard.type(lines[i]);
                if (i < lines.length - 1) {
                    await page.keyboard.down('Shift');
                    await page.keyboard.press('Enter');
                    await page.keyboard.up('Shift');
                }
            }
        }
        
        await page.keyboard.press('Enter');
        
        // Timeout allongé le temps que la puce TPUs/Gemini réponde sur l'interface (20s)
        await new Promise(resolve => setTimeout(resolve, 20000));
        
        const responseText = await page.evaluate(() => {
             // Extraction ciblée pour éviter d'aspirer l'interface de Google
             // On cherche les bulles de chat (contenant souvent 'message' ou 'response')
             const elements = Array.from(document.querySelectorAll('div'));
             const chatNodes = elements.filter(el => {
                 const className = (el.className || '').toLowerCase();
                 return (className.includes('message') || className.includes('response') || className.includes('chat')) 
                        && el.innerText.length > 50 
                        && el.offsetParent !== null; // visible
             });
             
             if (chatNodes.length > 0) {
                 // Le dernier noeud est généralement la réponse la plus récente
                 return chatNodes[chatNodes.length - 1].innerText.trim();
             }
             
             return "[Erreur d'extraction : Impossible de trouver le bloc de réponse AI. Le DOM a changé. Chef, il faudra m'inspecter la classe CSS de la bulle NotebookLM !]";
        });
        
        return responseText;
    } catch (error) {
        // Purge d'urgence de l'onglet en cas de désync DOM
        try { await page.close(); } catch(e) {}
        activePages.delete(name);
        throw new Error(`[Erreur DOM NotebookLM : ${name}] ` + error.message);
    }
}

server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const consultants = loadConsultants();
    const toolName = request.params.name;

    if (toolName === "list_consultants") {
        return { content: [{ type: "text", text: JSON.stringify(consultants, null, 2) }] };
    }

    if (toolName === "add_consultant") {
        const { name, url } = request.params.arguments;
        consultants[name] = { url, enabled: true };
        saveConsultants(consultants);
        return { content: [{ type: "text", text: `Consultant ${name} ajouté avec succès. Relancez l'UI pour mapper l'outil.` }] };
    }

    if (toolName === "remove_consultant") {
        const { name } = request.params.arguments;
        if (consultants[name]) {
            delete consultants[name];
            saveConsultants(consultants);
            if (activePages.has(name)) {
                try { await activePages.get(name).close(); } catch(e){}
                activePages.delete(name);
            }
            return { content: [{ type: "text", text: `Consultant ${name} détruit de la sphère de contexte.` }] };
        }
        throw new Error(`Le Consultant ${name} est introuvable.`);
    }

    if (toolName === "toggle_consultant") {
        const { name, enabled } = request.params.arguments;
        if (consultants[name]) {
            consultants[name].enabled = enabled;
            saveConsultants(consultants);
            return { content: [{ type: "text", text: `Consultant ${name} activé: ${enabled}` }] };
        }
        throw new Error(`Le Consultant ${name} n'existe pas.`);
    }

    // Le moteur dynamique (ask_NAME)
    if (toolName.startsWith("ask_")) {
        const reqName = toolName.substring(4); 
        const nameToUse = Object.keys(consultants).find(k => k.toLowerCase() === reqName);
        
        if (!nameToUse || !consultants[nameToUse].enabled) {
            throw new Error(`Le consultant '${toolName}' n'est pas actif ou introuvable.`);
        }
        
        const { prompt } = request.params.arguments;
        const answer = await queryNotebookLMStateful(nameToUse, consultants[nameToUse].url, prompt);
        return { content: [{ type: "text", text: answer }] };
    }
    
    // Routage Fallback (Legacy pour les scripts existants)
    if (toolName === "ask_consultant") {
        const { name, prompt } = request.params.arguments;
        const nameToUse = Object.keys(consultants).find(k => k.toLowerCase() === name.toLowerCase());
        
        if (!nameToUse || !consultants[nameToUse].enabled) {
            throw new Error(`Consultant '${name}' inactif ou introuvable.`);
        }
        
        const answer = await queryNotebookLMStateful(nameToUse, consultants[nameToUse].url, prompt);
        return { content: [{ type: "text", text: answer }] };
    }
    
    throw new Error(`Fonction/Outil inconnu(e): ${toolName}`);
});

async function main() {
    const transport = new StdioServerTransport();
    await server.connect(transport);
    console.error("🚀 SovereignConsultantBridge v2.0 (Stateful Tabs / Dynamic Tools MCP) actif. En écoute.");
}

if (process.argv[1] && process.argv[1] === fileURLToPath(import.meta.url)) {
    main().catch(console.error);
}
