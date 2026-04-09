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

// Algorithme de Transposition Virtuelle (Bwd Pass)
// Décompresse W_quant et accumule `grad_x = dY * W^T`.
// Optimisation L1-Cache: Les threads d'un même demi-warp (16 threads)
// demandent exactement le même bloc `TernaryBlock16` (4 Bytes). 
// L'architecture Ampere broadcast automatiquement ce Hit L1 
// évitant ainsi le "Strided Access" mortel d'une lecture colonne habituelle.
__global__ void bitnet_bwd_dx_matmul(
    const float* __restrict__ dy,              // Gradients sortants F32 (M x N)
    const TernaryBlock16* __restrict__ weight, // Poids quantifiés (N x (K/16))
    float* __restrict__ grad_x,                // Gradients des activations F32 (M x K)
    int M, int N, int K,
    int stride_dy_m, int stride_dy_n,
    int stride_w_n, int stride_w_k)
{
    int row = blockIdx.y * blockDim.y + threadIdx.y;
    int col_k = blockIdx.x * blockDim.x + threadIdx.x;

    if (row < M && col_k < K) {
        float acc = 0.0f;
        int block_k = col_k / 16;
        int bit_idx = col_k % 16;

        for (int n = 0; n < N; ++n) {
            // Lecture L1 Coalesced avec strides VRAM exacts
            TernaryBlock16 w_block = weight[n * stride_w_n + block_k * stride_w_k];
            // Lecture L1 Coalesced dY avec strides VRAM exacts
            float dy_val = dy[row * stride_dy_m + n * stride_dy_n];

            uint16_t bit_pos = (w_block.m_pos >> bit_idx) & 1;
            uint16_t bit_neg = (w_block.m_neg >> bit_idx) & 1;

            if (bit_pos) acc += dy_val;
            if (bit_neg) acc -= dy_val;
        }

        grad_x[row * K + col_k] = acc;
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
