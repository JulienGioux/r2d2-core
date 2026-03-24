#!/usr/bin/env bash
################################################################################
# R2D2 FORGE - DEVTOOLS: MCP Ping & Diagnostic Tool
################################################################################
#
# Ce script permet de vérifier la santé du serveur Axum R2D2-MCP
# en lui envoyant une requête CallTool (diagnostic) minimale conforme JSON-RPC.
# Utile pour valider que le réseau est live avant de lancer des ingestions lourdes.
#
# Utilisation : ./trigger_rpc.sh [PORT]
################################################################################

PORT=${1:-3030}
ENDPOINT="http://127.0.0.1:${PORT}/mcp"

echo "🔍 Envoi de la requête de statut MCP à ${ENDPOINT}..."

curl -s -X POST $ENDPOINT \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "call_tool",
    "params": {
      "name": "system_diagnostic",
      "arguments": {
        "level": "verbose"
      }
    },
    "id": "ping_1"
  }' | jq '.' || echo "\n❌ Impossible de formater le JSON ou le serveur est mort."
