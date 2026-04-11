#!/usr/bin/env bash
set -e

# ==============================================================================
# Script de Distribution : R2D2-Vampire & R2D2-Bridge
# Déploiement générique et multi-plateforme.
# ==============================================================================

# 1. Détection dynamique des chemins hôtes (Linux / WSL / Windows)
# Découverte Heuristique de PowerShell (Support pwsh 7 + WSL fallback)
get_powershell() {
    if command -v pwsh.exe &> /dev/null; then echo "pwsh.exe"; return; fi
    if command -v powershell.exe &> /dev/null; then echo "powershell.exe"; return; fi
    
    PWSH_PATH=$(ls /mnt/c/Program\ Files/PowerShell/*/pwsh.exe 2>/dev/null | head -n 1)
    if [ -n "$PWSH_PATH" ]; then echo "$PWSH_PATH"; return; fi
    
    echo "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
}

PWSH_CMD=$(get_powershell)

if [ -x "$PWSH_CMD" ] || command -v "$PWSH_CMD" &> /dev/null; then
    WIN_LOCALAPPDATA=$("$PWSH_CMD" -NoProfile -Command 'Write-Host -NoNewline $env:LOCALAPPDATA' | tr -d '\r')
    WSL_ANTIGRAVITY_PATH="$(wslpath -u "$WIN_LOCALAPPDATA")/Programs/Antigravity"
else
    # Fallback pur Linux si même l'interop brute échoue
    WSL_ANTIGRAVITY_PATH="$HOME/.local/share/Antigravity"
fi

ANTIGRAVITY_MCP_DIR="$HOME/.gemini/antigravity/mcp"
CARGO_TARGET_DIR="target"

echo "📦 1. Build du Serveur MCP R2D2-Vampire (Linux/WSL)..."
cargo build --release -p r2d2-vampire

echo "🚀 2. Déploiement du Serveur MCP pour Antigravity..."
mkdir -p "$ANTIGRAVITY_MCP_DIR"
cp "$CARGO_TARGET_DIR/release/server" "$ANTIGRAVITY_MCP_DIR/r2d2-vampire-server"

echo "📦 3. Envoi du pont R2D2-Bridge (Windows)..."
# Note: R2D2-Bridge doit être compilé depuis un terminal Windows natif ou cross-compilé : 'cargo build --release -p r2d2-bridge --target x86_64-pc-windows-msvc'
BRIDGE_TARGET="$CARGO_TARGET_DIR/x86_64-pc-windows-msvc/release/r2d2-bridge.exe"

if [ -f "$BRIDGE_TARGET" ]; then
    mkdir -p "$WSL_ANTIGRAVITY_PATH/target/x86_64-pc-windows-msvc/release/"
    cp "$BRIDGE_TARGET" "$WSL_ANTIGRAVITY_PATH/target/x86_64-pc-windows-msvc/release/r2d2-bridge.exe"
    echo "✅ R2D2-Bridge déployé dynamiquement côté Windows ($WSL_ANTIGRAVITY_PATH)."
else
    echo "⚠️ R2D2-Bridge non détecté ($BRIDGE_TARGET)."
    echo "    -> Assurez-vous d'avoir exécuté la compilation MSVC au préalable."
fi

echo "✅ Le pack distribué R2D2-Vampire est prêt et installé. Indépendant de la machine hôte."


