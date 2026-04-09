#[cfg(test)]
mod tests {
    use r2d2_cuda_core::CHIMERA_CUDA_KERNEL_SRC;

    #[test]
    fn test_cuda_kernel_source_integrity() {
        assert!(
            CHIMERA_CUDA_KERNEL_SRC.contains("bitnet_f32_dualmask_matmul"),
            "Le code source CUDA doit contenir le noyau bitnet_f32_dualmask_matmul"
        );
        assert!(
            CHIMERA_CUDA_KERNEL_SRC.contains("__global__"),
            "Le code source CUDA doit être un noyau valide"
        );
    }
}
