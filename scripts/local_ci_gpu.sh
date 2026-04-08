#!/bin/bash
set -e

echo "=========================================="
echo "🔴  R2D2 GPU Integration Pipeline (Industrial-Grade)"
echo "=========================================="

echo "[1/1] 🧪 Running GPU Test Suite (cargo test inside Podman)..."
# Validation des Kernels Cuda avec TDD Tensoriel dans un Podman isolé

if ! command -v podman &> /dev/null
then
    echo "❌ Podman introuvable. Ce script nécessite Podman pour virtualiser NVCC."
    exit 1
fi

echo "Déploiement du conteneur de test CUDA (avec Pass-Through CDI Rootless)..."
# On expose tous les GPUs à l'intérieur via --device nvidia.com/gpu=all
podman run --rm --device nvidia.com/gpu=all -v $(pwd):/workspace -w /workspace docker.io/nvidia/cuda:12.6.0-devel-ubuntu22.04 bash -c "apt-get update && apt-get install -y curl pkg-config libssl-dev && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source \$HOME/.cargo/env && cargo test -p r2d2-cuda-core --features 'cuda'"

echo "✅ GPU Tests OK."

echo ""
echo "=========================================="
echo "🚀 PASSED! CUDA GPU Core is mathematically safe."
echo "=========================================="
