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

// Fonction utilitaire pour la réduction au sein d'un Warp (32 threads)
// Obligatoire pour la scalabilité des accès "Coalesced" sur le bus VRAM
__device__ __forceinline__ float warpReduceSum(float val) {
    for (int offset = 16; offset > 0; offset /= 2)
        val += __shfl_down_sync(0xffffffff, val, offset);
    return val;
}

// Noyau de Routage SparseMoE (Top-1 Expert)
// Remplaçant de la fonction CPU "Rayon", 100% Asynchrone VRAM via GPU
// 1 Warp (32 threads) gère indépendamment l'évaluation d'un Token entier.
__global__ void sparse_moe_routing_top1(
    const float* __restrict__ act,             // Batch de Jetons [num_tokens, hidden_dim]
    const float* __restrict__ gate_weight,     // Poids routeurs [num_experts, hidden_dim]
    int32_t* __restrict__ expert_assignments,  // Sortie [num_tokens] (ID de l'expert)
    int num_tokens, 
    int num_experts, 
    int hidden_dim)
{
    // On groupe par Warp : 1 warp = 32 threads
    int global_thread_id = blockIdx.x * blockDim.x + threadIdx.x;
    int warp_id = global_thread_id / 32;
    int lane_id = global_thread_id % 32;

    if (warp_id >= num_tokens) return;

    int token_offset = warp_id * hidden_dim;

    float best_score = -1e38f; // Moins l'infini
    int best_expert = 0;

    for (int e = 0; e < num_experts; ++e) {
        int gate_offset = e * hidden_dim;
        float score = 0.0f;

        // Lecture Coalescée par 32 threads
        for (int i = lane_id; i < hidden_dim; i += 32) {
            float val = act[token_offset + i];
            float weight = gate_weight[gate_offset + i];
            
            // Routage de type BitNet "MatMul-Free"
            if (weight > 0.5f) {
                score += val;
            } else if (weight < -0.5f) {
                score -= val;
            }
        }

        // Warp Reduction
        score = warpReduceSum(score);

        // Le Leader du Warp consolide l'affinité
        if (lane_id == 0) {
            if (score > best_score) {
                best_score = score;
                best_expert = e;
            }
        }
    }

    if (lane_id == 0) {
        expert_assignments[warp_id] = best_expert;
    }
}

} // extern "C"
