#!/bin/bash
set -e

echo "=========================================="
echo "🛡️  R2D2 Local CI Pipeline (Industrial-Grade)"
echo "=========================================="

echo "[1/3] 💅 Applying Formatting (cargo fmt)..."
cargo fmt --all
echo "✅ Formatting OK."

echo ""
echo "[2/3] 🔍 Checking Lints (cargo clippy)..."
echo "Policy: Zero-Warning Policy (-D warnings)"
cargo clippy --workspace --all-targets --all-features -- -D warnings
echo "✅ Lints OK."

echo ""
echo "[3/3] 🧪 Running Test Suite (cargo test)..."
cargo test --workspace --all-features
echo "✅ Tests OK."

echo ""
echo "=========================================="
echo "🚀 PASSED! Code is safe to push."
echo "=========================================="
