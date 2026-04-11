#!/bin/bash
set -e

echo "🦇 [1/3] Compilation du SuperServeur R2D2-Vampire (Mode Release Extrême)..."
cargo build --release

echo "🦇 [2/3] Patching du Cœur d'Antigravity (mcp_config.json)..."
CONFIG_FILE="$HOME/.gemini/antigravity/mcp_config.json"

if [ -f "$CONFIG_FILE" ]; then
    # Utilisation de Node.js (Déjà présent sur le système de développement) pour une édition JSON sûre
    node -e "
        const fs = require('fs');
        const path = '$CONFIG_FILE';
        let data = JSON.parse(fs.readFileSync(path, 'utf8'));
        
        // 1. Désactivation sécurisée de l'ancien pont Node JS
        if (data.mcpServers && data.mcpServers.notebooklm_bridge) {
            data.mcpServers.notebooklm_bridge.disabled = true;
        }
        
        // 2. Établissement de la Tête de Pont Native (via WSL.exe) avec Vault Hardware-Bound local
        if (!data.mcpServers) {
            data.mcpServers = {};
        }
        data.mcpServers.r2d2_vampire = {
            command: 'wsl.exe',
            args: [
                '-d', 'fedoraremix',
                '-e', '/home/jgx/source/R2D2/target/release/server',
                '--mode', 'json'
            ],
            env: {
                GITHUB_TOKEN: process.env.GITHUB_TOKEN || ''
            }
        };
        
        fs.writeFileSync(path, JSON.stringify(data, null, 2));
    "
    echo "✅ Configuration Antigravity écrasée avec succès. L'ancien pont est désactivé, le nouveau est actif."
else
    echo "⚠️ Erreur ! Fichier mcp_config.json introuvable dans $HOME/.gemini/antigravity/"
    exit 1
fi

echo "🚀 [3/3] Déploiement terminé !"
echo "---> Veuillez redémarrer l'Agent (Antigravity/Cursor) pour qu'il s'interface avec le réseau binaire."
