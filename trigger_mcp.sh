#!/bin/bash
cd /mnt/d/XXXX/R2D2
source ~/.cargo/env
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "anchor_thought", "arguments": {"content": "Le système autonome R2D2 est officiellement opérationnel. La passerelle MCP est stabilisée en mode natif Stdio JSON-RPC. Le moteur tensoriel local utilise le modèle Multilingual-E5-Small 384D (Zero-Padded à 1024D) pour alimenter la résolution HNSW de la base de données PostgreSQL.", "agent_name": "Agent IA Architecte (Gemini)"}}}' | ./target/debug/r2d2-mcp > mcp_response.json 2> mcp_error.log
