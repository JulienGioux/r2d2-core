#!/bin/bash
# R2D2 Automated Run Audit Script for Antigravity Agent
# Location: devtools/audit_run.sh
# Usage: ./audit_run.sh

# Move to the project root assuming script is in devtools
cd "$(dirname "$0")/.."

LOG_DIR="logs"
LOG_FILE=$(ls -1t $LOG_DIR/r2d2*.log 2>/dev/null | head -n 1)

if [ -z "$LOG_FILE" ]; then
    echo "No log files found in $LOG_DIR/"
    exit 1
fi

echo "========================================="
echo "        R2D2 SYSTEM AUDIT REPORT         "
echo "========================================="
echo "Log File: $LOG_FILE"
echo "Last Modified: $(stat -c %y "$LOG_FILE")"
echo "Size: $(du -h "$LOG_FILE" | cut -f1)"
echo "-----------------------------------------"

echo -e "\n🔴 [CRITICAL ERRORS & PANICS]"
grep -i -E "error|panic|failed|exception" "$LOG_FILE" | tail -n 15 || echo "  > No recent errors detected."

echo -e "\n🟠 [WARNINGS & RATE LIMITS/FAILOVERS]"
grep -i -E "warn|ratelimit|quota|limit|fallback|timeout" "$LOG_FILE" | tail -n 15 || echo "  > No warnings detected."

echo -e "\n🧠 [AGENT COGNITION & INFERENCE]"
grep -E "ReasoningAgent|Inférence|mistral|gemini|ParadoxEngine|Démarrage de la boucle" "$LOG_FILE" | tail -n 10 || echo "  > No inference logs."

echo -e "\n🔧 [MCP & TOOL ACTIVITY]"
grep -i -E "mcp|tool|github_mcp|notebooklm|Tool" "$LOG_FILE" | tail -n 15 || echo "  > No MCP tool executions."

echo -e "\n📌 [LAST 15 SYSTEM EVENTS]"
tail -n 15 "$LOG_FILE"
echo -e "\n========================================="
echo "            END OF REPORT                "
echo "========================================="
