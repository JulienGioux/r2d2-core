#!/usr/bin/env bash
# ==============================================================================
# Script de configuration du pont CDP (Chrome DevTools Protocol) pour WSL2.
# Fournit l'accès à Chrome Windows depuis un environnement WSL.
#
# Architecture : WSL 127.0.0.1:9222 -> socat -> hôte Windows NAT -> portproxy -> Chrome
# ==============================================================================
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}======================================================="
echo -e "   Sovereign Forge : Relais CDP Chrome pour WSL2"
echo -e "=======================================================${NC}"

# 1. Vérification WSL
if ! grep -qi "microsoft" /proc/version; then
    echo -e "${RED}Erreur : Ce script doit être exécuté dans un environnement WSL2.${NC}"
    exit 1
fi

if command -v powershell.exe &> /dev/null; then
    POWERSHELL_BIN="powershell.exe"
elif command -v pwsh.exe &> /dev/null; then
    POWERSHELL_BIN="pwsh.exe"
elif [ -x "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe" ]; then
    POWERSHELL_BIN="/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
elif [ -x "/mnt/c/Windows/SysWOW64/WindowsPowerShell/v1.0/powershell.exe" ]; then
    POWERSHELL_BIN="/mnt/c/Windows/SysWOW64/WindowsPowerShell/v1.0/powershell.exe"
else
    echo -e "${RED}Erreur : powershell.exe introuvable. Vérifiez que l'interopérabilité Windows WSL est activée.${NC}"
    exit 1
fi

# 2. Installer socat de manière multi-distro
install_socat() {
    if command -v socat &> /dev/null; then
        echo -e "${GREEN}✓ socat est déjà installé.${NC}"
        return 0
    fi
    
    echo -e "${YELLOW}► L'utilitaire socat est manquant. Il est nécessaire pour relayer le trafic TCP.${NC}"
    read -p "Voulez-vous l'installer maintenant ? [O/n] " prompt
    if [[ $prompt == "n" || $prompt == "N" ]]; then
        echo -e "${RED}Installation annulée. Fin du script.${NC}"
        exit 1
    fi

    echo -e "${BLUE}Installation de socat...${NC}"
    if [ -f /etc/debian_version ]; then
        sudo apt-get update && sudo apt-get install -y socat
    elif [ -f /etc/redhat-release ] || command -v dnf &> /dev/null || command -v yum &> /dev/null; then
        if command -v dnf &> /dev/null; then
            sudo dnf install -y socat
        else
            sudo yum install -y socat
        fi
    elif [ -f /etc/arch-release ] || command -v pacman &> /dev/null; then
        sudo pacman -Sy --noconfirm socat
    elif command -v zypper &> /dev/null; then
        sudo zypper install -y socat
    else
        echo -e "${RED}Distribution non reconnue. Veuillez installer 'socat' manuellement.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ socat a été installé avec succès.${NC}"
}
install_socat

# 3. Récupérer l'IP de la passerelle
GATEWAY_IP=$(ip route show | grep -i default | awk '{ print $3}')
if [ -z "$GATEWAY_IP" ]; then
    echo -e "${RED}Impossible de déterminer l'adresse IP de la passerelle Windows.${NC}"
    exit 1
fi
echo -e "${GREEN}✓ IP Passerelle détectée : ${GATEWAY_IP}${NC}"

# 4. Autoriser / Forward les ports sur l'hôte Windows
echo -e "\n${BLUE}► Configuration du Proxy réseau et du Pare-feu Windows...${NC}"
echo -e "${YELLOW}ℹ️  Une alerte de l'UAC Windows va vous demander l'élévation des privilèges administrateur.${NC}"

TMP_PS1=$(mktemp --suffix=.ps1)
WIN_TMP_PS1=$(wslpath -w "$TMP_PS1")

cat <<EOF > "$TMP_PS1"
\$ErrorActionPreference = 'Stop'
try {
    Write-Host "Mise en place du portproxy Windows..."
    netsh interface portproxy delete v4tov4 listenport=9222 listenaddress=$GATEWAY_IP 2> \$null
    netsh interface portproxy add v4tov4 listenport=9222 listenaddress=$GATEWAY_IP connectport=9222 connectaddress=127.0.0.1
    
    Write-Host "Verification du Pare-feu..."
    if (-not (Get-NetFirewallRule -DisplayName "Chrome Remote Debug" -ErrorAction SilentlyContinue)) {
        New-NetFirewallRule -DisplayName "Chrome Remote Debug" -Direction Inbound -LocalPort 9222 -Protocol TCP -Action Allow > \$null
        Write-Host "Regle Pare-feu creee."
    }
} catch {
    Write-Error \$_
    Start-Sleep -Seconds 5
}
EOF

"$POWERSHELL_BIN" -NoProfile -Command "Start-Process -WorkingDirectory 'C:\' -FilePath (Get-Process -Id \$PID).Path -Wait -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"$WIN_TMP_PS1\"'"
rm -f "$TMP_PS1"
echo -e "${GREEN}✓ Routage de l'hôte configuré.${NC}"

# 5. Démarrer Chrome correctement configuré
echo -e "\n${CYAN}================ Navigateur Windows ==============${NC}"
echo -e "Le proxy requiert que Chrome soit démarré avec des flags stricts."
echo -e "${RED}ATTENTION : Chrome doit être complètement fermé avant de continuer.${NC}"
echo -e "${YELLOW}Assurez-vous de fermer TOUTES vos fenêtres Chrome, y compris celles tournant en arrière-plan (barre des tâches).${NC}"
read -p "Appuyez sur Entrée lorsque vous êtes prêt à démarrer Chrome (ou tapez 's' pour sauter cette étape) : " chrome_choice

if [[ "$chrome_choice" != "s" && "$chrome_choice" != "S" ]]; then
    TMP_CHROME_PS1=$(mktemp --suffix=.ps1)
    WIN_TMP_CHROME_PS1=$(wslpath -w "$TMP_CHROME_PS1")
    cat <<EOF > "$TMP_CHROME_PS1"
    \$chromePath = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe' -ErrorAction SilentlyContinue).'(default)'
    if (-not \$chromePath) { \$chromePath = "C:\Program Files\Google\Chrome\Application\chrome.exe" }
    
    # Isoler l'instance pour ne pas interférer avec le Chrome de l'utilisateur ou refuser l'ouverture
    \$profilePath = "\$Env:LOCALAPPDATA\Chrome_WSL_Debug"
    Start-Process -WorkingDirectory 'C:\' -FilePath "\$chromePath" -ArgumentList '--remote-debugging-port=9222', '--remote-allow-origins=*', '--no-first-run', '--no-default-browser-check', "--user-data-dir=\$profilePath"
EOF
    "$POWERSHELL_BIN" -NoProfile -ExecutionPolicy Bypass -File "$WIN_TMP_CHROME_PS1"
    rm -f "$TMP_CHROME_PS1"
    echo -e "${GREEN}✓ Chrome relancé en mode debug.${NC}"
    echo -e "${YELLOW}On patiente 3 secondes pour laisser le serveur CDP s'initialiser...${NC}"
    sleep 3
else
    echo -e "${YELLOW}► Démarrage automatisé de Chrome ignoré.${NC}"
fi

# 6. Démarrage de socat de ce côté (WSL)
echo -e "\n${CYAN}================ Relais WSL interne ==============${NC}"
if fuser 9222/tcp >/dev/null 2>&1; then
    echo -e "${YELLOW}► Un processus socat écoute déjà. Faisons le ménage (libération du port)...${NC}"
    fuser -k 9222/tcp >/dev/null 2>&1 || true
    sleep 1
fi

echo -e "${BLUE}Démarrage du processus relais (socat TCP-LISTEN)...${NC}"
socat TCP-LISTEN:9222,fork,bind=127.0.0.1,reuseaddr TCP:${GATEWAY_IP}:9222 > /dev/null 2>&1 &
SOCAT_PID=$!
sleep 1

# 7. Validation finale
echo -e "\n${CYAN}================ Tests & Validation ==============${NC}"
if curl -s http://127.0.0.1:9222/json/version | grep -qi "Browser"; then
    echo -e "${GREEN}✓ SUCCÈS ABSOLU ! Le navigateur de Windows répond à WSL sur localhost (9222).${NC}"
    echo -e "L'agent (Antigravity/Playwright/NotebookLM) peut s'y attacher sans la moindre barrière.${NC}"
    echo -e "\n${YELLOW}ℹ️  Le processus local tourne en arrière-plan (PID: ${SOCAT_PID}).${NC}"
    echo -e "Vous pouvez fermer ou utiliser ce terminal normalement.${NC}"
else
    echo -e "${RED}✗ Échec de la connexion. Chrome ne répond pas de l'autre côté.${NC}"
    echo -e "Pistes de résolution :"
    echo -e "  - Avez-vous bien validé la fenêtre 'Administrateur' Windows (UAC) ?"
    echo -e "  - Avez-vous bien fermé TOUS les onglets Chrome avant son relancement ?"
    echo -e "  - Utilisez-vous un VPN ou un antivirus bloquant le trafic local ?"
    kill $SOCAT_PID >/dev/null 2>&1 || true
    exit 1
fi
