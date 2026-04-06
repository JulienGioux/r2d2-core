#!/usr/bin/env bash

# R2D2 Launch Script
# Usage: ./launch.sh [--dev|--test|--prod] [-d|--daemon]

# Default values
MODE="prod"
DAEMON=0
LOG_DIR="logs"
FEATURE_FLAG=""
CUDA_DETECTED=0
METAL_DETECTED=0

# Ensure logic is run from script directory
cd "$(dirname "$0")"

# === 1. PARSE ARGUMENTS ===
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --dev) MODE="dev" ;;
        --test) MODE="test" ;;
        --prod) MODE="prod" ;;
        --cuda) CUDA_DETECTED=1 ;;
        --metal) METAL_DETECTED=1 ;;
        -d|--daemon) DAEMON=1 ;;
        *) echo "Unknown parameter passed: $1"; exit 1 ;;
    esac
    shift
done

echo -e "\e[1;36m[+] Initializing R2D2 in $MODE mode...\e[0m"

# === 2. PREREQUISITES & PHANTOM KILLER ===
echo -e "\e[1;34m[*] Checking for phantom processes on port 3000 or r2d2-ui...\e[0m"
PIDS=$(lsof -ti:3000 2>/dev/null)
PIDS="$PIDS $(pgrep r2d2-ui 2>/dev/null)"
PIDS=$(echo "$PIDS" | tr ' ' '\n' | sort -u | grep -v '^$')

if [ -n "$PIDS" ]; then
    echo -e "\e[1;33m[!] Phantom R2D2 processes detected (PIDs: $(echo $PIDS | tr '\n' ' ')).\e[0m"
    echo -ne "Do you want to kill them? [Y/n] (Auto-kill in 10s): "
    read -t 10 response
    if [[ -z "$response" || "$response" =~ ^[Yy]$ ]]; then
        echo -e "\n\e[1;31m[+] Killing processes: $PIDS\e[0m"
        kill -9 $PIDS 2>/dev/null || true
    else
        echo -e "\n\e[1;32m[*] Skipping cleanup.\e[0m"
    fi
else
    echo -e "\e[1;32m[*] No phantom processes found.\e[0m"
fi

# === 3. DOCKER / PODMAN AGNOSTIC DETECTION ===
echo -e "\e[1;34m[*] Verifying container engine...\e[0m"
COMPOSE_CMD=""
if command -v podman-compose >/dev/null 2>&1; then
    COMPOSE_CMD="podman-compose"
elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD="docker-compose"
elif command -v podman >/dev/null 2>&1; then
    COMPOSE_CMD="podman compose"
elif command -v docker >/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
else
    echo -e "\e[1;31m[!] Neither Podman nor Docker was found. Exiting.\e[0m"
    exit 1
fi

echo -e "\e[1;32m[*] Using container engine: $COMPOSE_CMD\e[0m"
echo -e "\e[1;34m[*] Starting R2D2 Database Container...\e[0m"
$COMPOSE_CMD up -d postgres

# === 4. HARDWARE ACCELERATION CHECK ===
if [ $CUDA_DETECTED -eq 1 ]; then
    echo -e "\e[1;33m[*] Verifying CUDA Toolkit Presence...\e[0m"
    if ! command -v nvcc >/dev/null 2>&1; then
        echo -e "\e[1;31m[!] NVIDIA CUDA Compiler (nvcc) not found!\e[0m"
        echo -ne "\e[1;33m[?] Do you want R2D2 to automatically install the CUDA Toolkit via DNF? (Requires sudo) [Y/n]: \e[0m"
        read -r response
        if [[ -z "$response" || "$response" =~ ^[Yy]$ ]]; then
            echo -e "\e[1;34m[*] Installing constraints and repositories...\e[0m"
            sudo dnf install -y gcc gcc-c++ dnf-plugins-core
            
            # Dynamic OS Version detection for Nvidia Repo
            F_VER=$(source /etc/os-release && echo $VERSION_ID)
            # Nvidia is usually late. Cap at 40 if system is newer.
            if [ "$F_VER" -gt 40 ]; then
                F_VER=40
            fi
            
            REPO_URL="https://developer.download.nvidia.com/compute/cuda/repos/fedora${F_VER}/x86_64/cuda-fedora${F_VER}.repo"
            echo -e "\e[1;34m[*] Adding dynamic Nvidia repo: $REPO_URL\e[0m"
            sudo dnf config-manager --add-repo "$REPO_URL"
            
            sudo dnf clean all
            echo -e "\e[1;34m[*] Downloading and installing the full CUDA Toolkit (this may take a while)...\e[0m"
            sudo dnf install -y cuda-toolkit
            
            echo -e "\e[1;32m[+] Installation Complete!\e[0m"
            echo -e "\e[1;31m[!] IMPORTANT: The PATH variables have been updated. You MUST close this terminal and open a new one for 'nvcc' to be recognized.\e[0m"
            echo -e "\e[1;31m[!] After reopening, execute: ./launch.sh --cuda\e[0m"
            exit 0
        else
            echo -e "\e[1;31m[!] Launch aborted. Cannot compile features=['cuda'] without nvcc.\e[0m"
            exit 1
        fi
    fi
    echo -e "\e[1;32m[+] CUDA Toolkit Detected. Enabling --features cuda\e[0m"
    FEATURE_FLAG="--features r2d2-cortex/cuda"
elif [ $METAL_DETECTED -eq 1 ]; then
    echo -e "\e[1;32m[+] Enabling Apple Silicon Metal Support. Enabling --features metal\e[0m"
    FEATURE_FLAG="--features r2d2-cortex/metal"
fi

# === 5. LAUNCH LOGIC ===
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/r2d2_ui_$(date +%F).log"

if [ "$MODE" == "test" ]; then
    echo -e "\e[1;36m[+] Running tests...\e[0m"
    export RUST_LOG=debug
    cargo test $FEATURE_FLAG
    exit $?
fi

if [ "$MODE" == "dev" ]; then
    echo -e "\e[1;36m[+] Starting in DEV mode (cargo run)...\e[0m"
    CMD="cargo run $FEATURE_FLAG -p r2d2-ui"
    export RUST_LOG=debug,r2d2_ui=debug,r2d2_cortex=debug
else
    echo -e "\e[1;36m[+] Starting in PROD mode...\e[0m"
    echo -e "\e[1;34m[*] Compiling release build...\e[0m"
    if ! cargo build --release $FEATURE_FLAG -p r2d2-ui; then
        echo -e "\e[1;31m[!] BUILD FAILED. Please resolve errors before running.\e[0m"
        $COMPOSE_CMD stop postgres
        exit 1
    fi
    CMD="./target/release/r2d2-ui"
    export RUST_LOG=info
fi

if [ $DAEMON -eq 1 ]; then
    echo -e "\e[1;32m[+] Launching in daemon mode. Logs available at $LOG_FILE\e[0m"
    nohup $CMD > "$LOG_FILE" 2>&1 &
    NEW_PID=$!
    echo -e "\e[1;32m[+] R2D2 is running (PID: $NEW_PID).\e[0m"
else
    trap 'echo -e "\n\e[1;34m[*] Stopping containers (Trap)...\e[0m"; $COMPOSE_CMD stop postgres; exit 0' EXIT INT TERM
    echo -e "\e[1;33m[+] Launching in foreground, press Ctrl+C to stop.\e[0m"
    $CMD
fi
