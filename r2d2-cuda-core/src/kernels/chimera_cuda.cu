#include <cuda_runtime.h>
#include <stdint.h>
#include <stdio.h>

extern "C" {

// Noyau PTX "Zéro-Crash" pour la passe Forward W1A8
__global__ void bitnet_matmul_kernel(const uint8_t* x, const uint8_t* w, float* out, size_t batch_seq, size_t hidden_dim, size_t intermediate_size) {
    int row = blockIdx.y * blockDim.y + threadIdx.y;
    int col = blockIdx.x * blockDim.x + threadIdx.x;

    if (row < batch_seq && col < intermediate_size) {
        int32_t acc = 0;
        const int8_t* sx = (const int8_t*)x;
        const int8_t* sw = (const int8_t*)w;
        
        // Boucle sans "MatMul" classique : uniquement des entiers et conditions simples.
        for (size_t k = 0; k < hidden_dim; ++k) {
            int8_t w_val = sw[col * hidden_dim + k];
            if (w_val != 0) {
                int8_t x_val = sx[row * hidden_dim + k];
                // Les GPU excellent sur PREDICATION (exécution sans saut de branchement)
                // Mais avec CUDA moderne, le compilateur PTX résoudra efficacement ce if-else
                if (w_val > 0) acc += x_val;
                else acc -= x_val;
            }
        }
        
        // La seule conversion en flottant intervient à l'export final de la somme.
        out[row * intermediate_size + col] = (float)acc;
    }
}

int32_t chimera_w1a8_matmul(const uint8_t* x, const uint8_t* w, float* out, size_t batch_seq, size_t hidden_dim, size_t intermediate_size) {
    dim3 threadsPerBlock(16, 16);
    dim3 numBlocks((intermediate_size + threadsPerBlock.x - 1) / threadsPerBlock.x,
                   (batch_seq + threadsPerBlock.y - 1) / threadsPerBlock.y);

    bitnet_matmul_kernel<<<numBlocks, threadsPerBlock>>>(x, w, out, batch_seq, hidden_dim, intermediate_size);
    
    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) {
        printf("[CORTEX ALERTE] Erreur du Driver dans chimera_w1a8_matmul : %s\n", cudaGetErrorString(err));
        return -1;
    }
    
    err = cudaDeviceSynchronize();
    if (err != cudaSuccess) {
        printf("[CORTEX ALERTE] Panique lors de la synchronisation de VRAM : %s\n", cudaGetErrorString(err));
        return -1;
    }

    return 0;
}

} // extern "C"
