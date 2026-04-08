#!/bin/bash
# =========================================================================
# 🛡️ SOVEREIGN BOOTSTRAPPER (R2D2) - Universal WSL/Linux Setup
# =========================================================================

set -e # Arrêt du script en cas d'erreur critique

# --- Couleurs ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# --- Variables Dynamiques ---
PROJECT_ROOT="$(pwd)"
USER_HOME="$HOME"

echo -e "${BLUE}================================================================${NC}"
echo -e "${BLUE}🛡️  INITIALISATION DE L'ARCHITECTE SOUVERAIN (R2D2 BOOTSTRAPPER)${NC}"
echo -e "${BLUE}================================================================${NC}"
echo -e "Analyse de votre environnement local..."
echo -e "Racine détectée : ${CYAN}${PROJECT_ROOT}${NC}"
echo -e "Dossier User    : ${CYAN}${USER_HOME}${NC}"
echo ""

# Fonction de prompt Y/N
ask_user() {
    local prompt="$1"
    while true; do
        read -p "$(echo -e ${YELLOW}${prompt} [y/n]: ${NC})" yn
        case $yn in
            [Yy]* ) return 0;;
            [Nn]* ) return 1;;
            * ) echo -e "${RED}Veuillez répondre yes ou no.${NC}";;
        esac
    done
}

# --- 1. Télémétrie & Sondage Infrastructure ---
echo -e "${CYAN}[1/5] Sondage Infrastructure (Isolation & Matériel)${NC}"

IS_WSL=0
IS_CONTAINER=0
PKG_MANAGER=""
HAS_NVIDIA_GPU=0

# A. Détection Isolation Hôte
if grep -qi "microsoft" /proc/version 2>/dev/null || grep -qi "wsl" /proc/version 2>/dev/null; then
    IS_WSL=1
    echo -e "🔹 Couche Virtuelle : ${GREEN}WSL (Windows Subsystem for Linux) détecté.${NC}"
fi

if [ -f "/.dockerenv" ] || [ -f "/run/.containerenv" ]; then
    IS_CONTAINER=1
    echo -e "${YELLOW}⚠️  ATTENTION : Script exécuté depuis un conteneur ! (Opérations système restreintes)${NC}"
else
    echo -e "🔹 Isolation        : ${GREEN}Système Hôte natif.${NC}"
fi

# B. Détection Matériel (GPU)
if [ -c "/dev/dxg" ] || [ -d "/proc/driver/nvidia" ] || (command -v lspci &>/dev/null && lspci | grep -i nvidia &>/dev/null); then
    HAS_NVIDIA_GPU=1
    echo -e "🔹 Matériel         : ${GREEN}Equipement NVIDIA détecté physiquement.${NC}"
else
    echo -e "🔹 Matériel         : Aucune carte NVIDIA détectée."
fi

# C. Sondage du Gestionnaire de Paquets (Le juge de paix universel)
if command -v dnf &> /dev/null; then
    PKG_MANAGER="dnf"
    pkg_update="sudo dnf update -y"
    pkg_install="sudo dnf install -y"
    base_deps="gcc gcc-c++ make cmake pkg-config openssl-devel podman curl git"
    echo -e "🔹 Gestionnaire     : ${GREEN}DNF (Famille RHEL/Fedora)${NC}"
elif command -v apt-get &> /dev/null; then
    PKG_MANAGER="apt"
    pkg_update="sudo apt-get update -y && sudo apt-get upgrade -y"
    pkg_install="sudo apt-get install -y"
    base_deps="build-essential cmake pkg-config libssl-dev podman curl git"
    echo -e "🔹 Gestionnaire     : ${GREEN}APT (Famille Debian/Ubuntu)${NC}"
elif command -v pacman &> /dev/null; then
    PKG_MANAGER="pacman"
    pkg_update="sudo pacman -Syu --noconfirm"
    pkg_install="sudo pacman -S --noconfirm"
    base_deps="base-devel cmake pkgconf openssl podman curl git"
    echo -e "🔹 Gestionnaire     : ${GREEN}Pacman (Famille Arch Linux)${NC}"
else
    echo -e "${RED}❌ Aucun gestionnaire de paquets reconnu (dnf, apt, pacman absents). Impossible de garantir un bootstrap sûr.${NC}"
    exit 1
fi

echo -e "Prérequis nécessaires au C/C++ et Rust : ${CYAN}${base_deps}${NC}"
if ask_user "Voulez-vous vérifier et installer les mises à jour et ces paquets systèmes (demande de mot de passe sudo) ?"; then
    echo -e "${CYAN}Mise à jour du cache de la distribution...${NC}"
    eval $pkg_update
    echo -e "${CYAN}Installation des fondamentaux...${NC}"
    eval "$pkg_install $base_deps"
    echo -e "${GREEN}Fondations Systèmes OK.${NC}"
else
    echo -e "${YELLOW}Étape ignorée. Assurez-vous d'avoir les compilateurs C++ à jour.${NC}"
fi
echo ""

# --- 2. Installation de Rust ---
echo -e "${CYAN}[2/5] Vérification de l'outil Rust (rustc/cargo)${NC}"
if command -v rustc &> /dev/null; then
    echo -e "${GREEN}Rust est déjà installé : $(rustc --version)${NC}"
else
    echo -e "${YELLOW}Rust n'est pas installé sur cet environnement.${NC}"
    if ask_user "Voulez-vous installer le Toolchain Rust via rustup (Indispensable pour R2D2) ?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        echo -e "${GREEN}Rust installé avec succès !${NC}"
        echo -e "${YELLOW}(NOTE: Vous devrez peut-être recharger votre terminal ou exécuter 'source $HOME/.cargo/env' plus tard)${NC}"
    else
        echo -e "${RED}Attention : R2D2 ne compilera pas sans Rust.${NC}"
    fi
fi
echo ""

# --- 3. Installation de Node.js (MCP) ---
echo -e "${CYAN}[3/5] Vérification de Node.js / NVM (Requis pour les agents MCP)${NC}"
# On charge nvm s'il existe
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

if command -v node &> /dev/null; then
    echo -e "${GREEN}Node.js est déjà installé : $(node -v)${NC}"
else
    echo -e "${YELLOW}Node.js n'est pas détecté.${NC}"
    if ask_user "Voulez-vous installer Node.js via NVM (Indispensable pour NotebookLM/GitHub MCP) ?"; then
        echo -e "Installation de NVM..."
        curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
        
        # Charger nvm immediatement pour la session courante
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

        echo -e "Installation de la version LTS de Node.js..."
        nvm install --lts
        echo -e "${GREEN}Node.js installé avec succès !${NC}"
    else
         echo -e "${YELLOW}Étape ignorée. Le Server MCP risque de ne pas démarrer.${NC}"
    fi
fi
echo ""


# --- 4. Pont Réseau WSL ---
echo -e "${CYAN}[4/5] Diagnostic WSL et Pont Réseau${NC}"
if [ "$IS_WSL" -eq 1 ]; then
    echo -e "Environnement WSL2 détecté."
    echo -e "Souvent, WSL ne résout pas bien les paquets réseaux du host Windows (ex: Ollama Host)."
    if ask_user "Voulez-vous installer l'injection automatique de l'IP Windows Host (WIN_HOST_IP) dans votre .bashrc ?"; then
        BASHRC="$HOME/.bashrc"
        if grep -q "WIN_HOST_IP=" "$BASHRC"; then
            echo -e "${YELLOW}Le pont réseau est déjà configuré dans le ~/.bashrc${NC}"
        else
            echo " " >> "$BASHRC"
            echo "# R2D2 - WSL Windows Host IP resolver" >> "$BASHRC"
            echo "export WIN_HOST_IP=\$(cat /etc/resolv.conf | grep nameserver | awk '{print \$2}')" >> "$BASHRC"
            echo -e "${GREEN}Injection réussie dans ~/.bashrc ! La modification prendra effet au prochain démarrage du terminal.${NC}"
        fi
    else
        echo -e "Étape ignorée."
    fi
else
    echo -e "Environnement Linux natif (Non-WSL). Pont Windows inutile."
fi
echo ""

# --- 5. Outils matériels : Cuda et CDI (Podman Pass-Through) ---
echo -e "${CYAN}[5/5] Matériel Accéléré (CUDA / CDI Rootless)${NC}"

echo -e "Analyse de la chaîne de compilation CUDA (NVCC)..."
if command -v nvcc &> /dev/null; then
    echo -e "${GREEN}Serveur CUDA (NVCC) détecté sur l'hôte : $(nvcc --version | head -n 1)${NC}"
else
    echo -e "${YELLOW}Aucun compilateur CUDA détecté.${NC}"
    echo -e "Attention : L'installation de CUDA Toolkits est très dépendante de vos drivers propriétaires et pèse plusieurs Go."
    if ask_user "Souhaitez-vous installer CUDA Toolkit maintenant avec le gestionnaire de paquets de la distribution ?"; then
            echo -e "${CYAN}Tentative d'installation de cuda-toolkit avec DNF 5...${NC}"
            sudo dnf config-manager addrepo --from-repofile=https://developer.download.nvidia.com/compute/cuda/repos/fedora42/x86_64/cuda-fedora42.repo
            sudo dnf install -y cuda-toolkit || echo -e "${RED}L'installation a échoué. Veuillez configurer le dépôt officiel NVIDIA.${NC}"
        elif [ "$PKG_MANAGER" = "apt" ]; then
            sudo apt-get install -y nvidia-cuda-toolkit
        else
            echo -e "Installation manuelle requise pour cette distribution."
        fi
    else
        echo -e "${YELLOW}CUDA ignoré. Le moteur R2D2 compilera sagement grâce au fallback CPU FFI.${NC}"
    fi
fi
echo ""

echo -e "${CYAN}Configuration du pont NVIDIA Container Toolkit (CDI) pour Podman...${NC}"
if ask_user "Installer le toolkit nvidia et configurer le pass-through matériel CDI (Indispensable pour le GPU en Rootless) ?"; then
    if ! command -v nvidia-ctk &> /dev/null; then
        if [ "$PKG_MANAGER" = "dnf" ]; then
            curl -s -L https://nvidia.github.io/libnvidia-container/stable/rpm/nvidia-container-toolkit.repo | sudo tee /etc/yum.repos.d/nvidia-container-toolkit.repo > /dev/null
            sudo dnf install -y nvidia-container-toolkit
        elif [ "$PKG_MANAGER" = "apt" ]; then
            curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
            curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
            sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
            sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list > /dev/null
            sudo apt-get update
            sudo apt-get install -y nvidia-container-toolkit
        fi
    fi

    if command -v nvidia-ctk &> /dev/null; then
        echo -e "Génération de la spécification CDI NVIDIA (/etc/cdi/nvidia.yaml)..."
        sudo nvidia-ctk cdi generate --output=/etc/cdi/nvidia.yaml
        echo -e "${GREEN}CDI nvidia configuré avec succès.${NC}"
    else
        echo -e "${RED}Échec de l'installation de nvidia-container-toolkit.${NC}"
    fi

    echo -e "${CYAN}Élévation des privilèges User Namespaces (newuidmap, newgidmap)...${NC}"
    sudo chmod +s /usr/bin/newuidmap /usr/bin/newgidmap 2>/dev/null || true
    echo -e "${GREEN}Droits ajustés pour l'isolation système.${NC}"
else
    echo -e "${YELLOW}Pass-Through ignoré.${NC}"
fi
echo ""

# --- 6. Post-installation (Droits & Scripts) ---
echo -e "${CYAN}[6/6] Opérations de post-restauration des fichiers R2D2${NC}"
echo "La purge des droits d'exécution (+x) est fréquente en cas de restauration WSL..."
if ask_user "Voulez-vous vérifier et restaurer automatiquement les droits d'exécution sur tous les scripts de ${PROJECT_ROOT} ?"; then
   find "${PROJECT_ROOT}" -type f -name "*.sh" -exec chmod +x {} \;
   echo -e "${GREEN}Droits restaurés pour les fichiers *.sh.${NC}"
fi

echo ""
echo -e "${BLUE}================================================================${NC}"
echo -e "${GREEN}✅ BOOTSTRAP TERMINÉ.${NC}"
echo -e "Votre terminal devra peut-être être relancé (Fermer et Rouvrir) pour"
echo -e "appliquer les modifications (Rust, Node, Réseau)."
echo -e "${BLUE}================================================================${NC}"
exit 0
