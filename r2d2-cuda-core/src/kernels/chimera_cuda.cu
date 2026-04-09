#include <cuda_runtime.h>
#include <stdint.h>
#include <stdio.h>

extern "C" {

struct TernaryBlock16 {
    uint16_t m_pos;
    uint16_t m_neg;
};

// Noyau PTX "Zéro-Crash" pour la passe Forward W1A8 (BitNet 1.58b)
// Activations en Float32, Poids Ternaires "Dual-Mask" (16 poids par 32-bits)
__global__ void bitnet_f32_dualmask_matmul(
    const float* __restrict__ act,             // Activations en F32 (M x K)
    const TernaryBlock16* __restrict__ weight, // Poids quantifiés en masques CPU/GPU (N x (K/16))
    float* __restrict__ out,                   // Sortie en F32 (M x N)
    int M, int N, int K)
{
    int row = blockIdx.y * blockDim.y + threadIdx.y;
    int col = blockIdx.x * blockDim.x + threadIdx.x;

    if (row < M && col < N) {
        float acc = 0.0f; // Accumulateur dans le registre local F32

        // La boucle K avance par pas de 16 (taille d'un TernaryBlock16)
        for (int k = 0; k < K; k += 16) {
            // Lecture coalescée d'un TernaryBlock contenant 16 poids
            // La matrice de poids est [N, K/16].
            TernaryBlock16 block = weight[col * (K / 16) + (k / 16)];

            uint16_t m_pos = block.m_pos;
            uint16_t m_neg = block.m_neg;

            #pragma unroll
            for (int i = 0; i < 16; ++i) {
                // Lecture de l'activation correspondante en F32
                float a_val = act[row * K + (k + i)];

                uint16_t bit_pos = (m_pos >> i) & 1;
                uint16_t bit_neg = (m_neg >> i) & 1;

                // LA MATHÉMATIQUE "MATMUL-FREE"
                // L'instruction CPU correspondante via les cœurs CUDA
                if (bit_pos) acc += a_val;
                if (bit_neg) acc -= a_val;
            }
        }
        out[row * N + col] = acc;
    }
}

// Fonction wrapper pour l'ancien FFI statique 
int32_t chimera_w1a8_matmul(const float* act, const TernaryBlock16* weight, float* out, int m, int n, int k) {
    dim3 threadsPerBlock(32, 32);
    dim3 numBlocks((n + threadsPerBlock.x - 1) / threadsPerBlock.x,
                   (m + threadsPerBlock.y - 1) / threadsPerBlock.y);

    bitnet_f32_dualmask_matmul<<<numBlocks, threadsPerBlock>>>(act, weight, out, m, n, k);
    
    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) {
        printf("[CORTEX ALERTE] Erreur du Driver Cuda : %s\n", cudaGetErrorString(err));
        return -1;
    }
    
    err = cudaDeviceSynchronize();
    if (err != cudaSuccess) {
        printf("[CORTEX ALERTE] Panique Sync VRAM : %s\n", cudaGetErrorString(err));
        return -1;
    }

    return 0;
}

} // extern "C"
