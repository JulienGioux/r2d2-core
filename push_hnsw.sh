#!/bin/bash
cargo fmt --all
git add .
git commit -m "feat(mcp): implement true pgvector hnsw sql search for recall_memory tool"
git push origin feat/phase-4-mcp-gateway
