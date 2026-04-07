#!/bin/bash
set -e

echo "=========================================="
echo "🛡️  R2D2 Local CI Pipeline (Industrial-Grade)"
echo "=========================================="

# La CI en ligne utilise souvent RUSTFLAGS pour injecter -D warnings globablement
export RUSTFLAGS="-D warnings"

echo "[1/3] 💅 Applying Formatting (cargo fmt)..."
cargo fmt --all
echo "✅ Formatting OK."

echo ""
echo "[2/3] 🔍 Checking Lints (cargo clippy)..."
echo "Policy: Zero-Warning Policy (-D warnings + clippy::all)"
# On force le compilateur à ignorer le cache silencieux local qui peut masquer des alertes
# en injectant RUSTFLAGS et en forçant les lints clippy complets
cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all
echo "✅ Lints OK."

echo ""
echo "[3/3] 🧪 Running Test Suite (cargo test)..."
cargo test --workspace --all-targets --all-features
echo "✅ Tests OK."

echo ""
echo "=========================================="
echo "🚀 PASSED! Code is safe to push."
echo "=========================================="
