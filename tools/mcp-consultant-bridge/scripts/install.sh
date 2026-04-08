#!/usr/bin/env bash
# Souverain Consultant Bridge - Autoinstallator

set -e

BRIDGE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MCP_CONFIG="$HOME/.gemini/antigravity/mcp_config.json"
MCP_SERVER_KEY="notebooklm_bridge"

echo "🤖 Installation du Souverain Consultant Bridge..."

# 1. Vérification des dépendances Node.js
cd "$BRIDGE_DIR"
if [ ! -d "node_modules" ]; then
    echo "📦 Installation des modules (Puppeteer-core & MCP SDK)..."
    npm install
fi

# 2. Injection automatique dans le mcp_config.json d'Antigravity
echo "⚙️  Configuration d'Antigravity MCP..."

if [ ! -f "$MCP_CONFIG" ]; then
    echo "Création de mcp_config.json vierge."
    echo '{"mcpServers": {}}' > "$MCP_CONFIG"
fi

# Utilisation de Node JSON parser pour écraser/ajouter le serveur
node -e "
const fs = require('fs');
const configPath = process.argv[1];
const bridgeDir = process.argv[2];

let config = { mcpServers: {} };
try {
    const raw = fs.readFileSync(configPath, 'utf8');
    config = JSON.parse(raw);
    if (!config.mcpServers) config.mcpServers = {};
} catch(e) { /* ignore parse error */ }

config.mcpServers['$MCP_SERVER_KEY'] = {
    command: 'node',
    args: [bridgeDir + '/index.js'],
    env: {}
};

fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
" "$MCP_CONFIG" "$BRIDGE_DIR"

echo "✅ Le Consultant Bridge a été injecté."
echo "⚠️ IMPORTANT : Redémarre ou recharge la fenêtre Antigravity pour activer l'Extension."
echo "Précision : Tu dois avoir lancé 'setup_wsl_chrome_cdp.sh' côté hôte si tu es sur WSL !"
