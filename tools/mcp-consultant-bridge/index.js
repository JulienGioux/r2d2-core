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
            
            const safePrompt = prompt.replace(/\r?\n/g, ' ');
            await chatEl.type(safePrompt);
        } else {
            // Fallback aveugle absolu
            const safePrompt = prompt.replace(/\r?\n/g, ' ');
            await page.keyboard.type(safePrompt);
        }
        
        // On compte le nombre de messages avant d'envoyer la question
        const initialCount = await page.evaluate(() => document.querySelectorAll('chat-message').length);
        
        await page.keyboard.press('Enter');

        let lastLength = -1;
        let stabilizeCount = 0;
        let finalResponse = "";

        // Boucle Flash (Vérification toutes les 3 secondes, Max 120s)
        for (let i = 0; i < 40; i++) {
            const result = await page.evaluate((ctx) => {
                const initCt = ctx.initCt;
                const promptString = ctx.promptStr || "";
                
                const messages = document.querySelectorAll('chat-message');
                
                // Google UI States
                const isGenerating = Array.from(document.querySelectorAll('[role="progressbar"], [aria-busy="true"], .generating'))
                    .some(l => l.offsetParent !== null);
                    
                const isReadySignal = Array.from(document.querySelectorAll('[aria-live="polite"]'))
                    .some(tag => tag.innerText && tag.innerText.toLowerCase().includes("prête"));

                // Logique Chirurgicale Zéro-Bloat (<chat-message> terminal)
                const chatMessages = Array.from(document.querySelectorAll('chat-message'));
                if (chatMessages.length === 0) return { missing: true, isLoading: isGenerating, bubbleText: "" };
                
                // On s'intéresse UNIQUEMENT au tout dernier message dans le DOM (celui qui est autofocus par Chrome)
                const lastMsg = chatMessages[chatMessages.length - 1];
                
                // ANTI-LAZY-LOADING: Forcer le scroll tout en bas du message pour empêcher 
                // l'UI Google de suspendre le flux Token-by-Token si le message est trop long.
                lastMsg.scrollIntoView({ behavior: 'auto', block: 'end' });
                
                let txt = lastMsg.innerText.trim();
                
                // Pour savoir si c'est la bulle de l'IA ou la nôtre, on compare les 30 premiers caractères 
                // (Cela nous immunise contre le "Show More" / truncate de Google)
                let pClean = promptString.trim().replace(/\s+/g, ' ').substring(0, 30);
                let msgClean = txt.replace(/\s+/g, ' ').substring(0, 30);

                if (msgClean.includes(pClean) || pClean.includes(msgClean)) {
                    // C'est TOUJOURS notre question ! L'IA n'a pas encore injecté sa réponse à la suite.
                    return { missing: true, isLoading: isGenerating, isReadySignal: false, bubbleText: "" };
                }

                // C'est la bulle de l'IA (On récupère tout le texte brut pour tracer le statut)
                let fullText = lastMsg.innerText.trim();
                let isReasoning = false;
                
                // La réponse finale doit obligatoirement être un bloc injecté par Angular (*ngIf -> ng-star-inserted)
                // Pour éviter de capturer l'UI des boutons (thumb_down, copy) qui sont aussi ng-star-inserted,
                // on filtre formellement en excluant les mots-clés UI ou par présence de balises Markdown.
                const ngInserted = Array.from(lastMsg.querySelectorAll('div.ng-star-inserted'))
                    .filter(d => {
                        const txtLocal = d.innerText.trim();
                        // Exclusion formelle des icones
                        if (txtLocal === "thumb_down" || txtLocal === "thumb_up" || txtLocal.includes("content_copy")) return false;
                        
                        // Si ça contient une structure Markdown, c'est obligatoirement la réponse
                        if (d.querySelectorAll('p, ul, ol, pre, code, blockquote').length > 0) return true;
                        
                        // Si le texte généré sans balise markdown fait plus de 60 caractères (impossible pour une icone)
                        if (txtLocal.length > 60) return true;
                        
                        return false;
                    });
                    
                let structuralAnswer = "";
                if (ngInserted.length > 0) {
                    // Le conteneur parent du Markdown est généralement le premier de la liste
                    structuralAnswer = ngInserted[0].innerText.trim();
                } else {
                    // S'il n'y a pas encore de bloc avec des paragraphes injecté, c'est obligatoirement du Reasoning
                    isReasoning = true;
                }

                return { 
                    missing: false, 
                    isLoading: isGenerating, 
                    isReasoning: isReasoning,
                    bubbleText: fullText, 
                    finalAnswer: structuralAnswer 
                };
            }, { initCt: initialCount, promptStr: prompt });

            let currentBubble = result.bubbleText;
            let currentAnswer = result.finalAnswer;

            // Si le bloc n'existe pas encore
            if (result.missing) {
                await new Promise(resolve => setTimeout(resolve, 3000));
                continue;
            }

            // Log des processus internes de Google (Reasoning)
            if (result.isReasoning || currentAnswer === "") {
                if (currentBubble.length !== lastLength && currentBubble.length > 0) {
                    if (currentBubble.length < 150) {
                        console.error(`[Expert en réflexion] ${currentBubble}`);
                    }
                    lastLength = currentBubble.length;
                }
                await new Promise(resolve => setTimeout(resolve, 3000));
                continue;
            }

            // Attente inconditionnelle de la fin du chargement Google (barre de chargement/spinners)
            if (result.isLoading) {
                lastLength = currentAnswer.length;
                await new Promise(resolve => setTimeout(resolve, 3000));
                continue;
            }

            // Quand le composant indique "terminé" et que la réponse "ng-star-inserted" est bien là
            if (currentAnswer.length === lastLength && currentAnswer.length > 0) {
                stabilizeCount++;
                if (stabilizeCount >= 2) { // 6 secondes de stabilité de la réponse finale
                    finalResponse = currentAnswer;
                    break;
                }
            } else {
                lastLength = currentAnswer.length;
                stabilizeCount = 0;
            }

            await new Promise(resolve => setTimeout(resolve, 3000));
        }

        if (!finalResponse) {
             throw new Error("[Erreur DOM NotebookLM : L'Agent a fait un Timeout à la génération. Vérifiez l'onglet manuellement.]");
        }
        
        return finalResponse;
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
