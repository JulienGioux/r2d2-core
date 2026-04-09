#[cfg(test)]
mod tests {
    #[cfg(feature = "cuda")]
    use r2d2_cuda_core::CHIMERA_CUDA_KERNEL_PTX;

    #[test]
    #[cfg(feature = "cuda")]
    fn test_cuda_kernel_source_integrity() {
        assert!(
            CHIMERA_CUDA_KERNEL_PTX.contains("bitnet_f32_u8_matmul"),
            "Le PTX généré doit contenir le noyau bitnet_f32_u8_matmul"
        );
        assert!(
            CHIMERA_CUDA_KERNEL_PTX.contains(".version"),
            "Le code source doit être un module PTX valide"
        );
    }
}
