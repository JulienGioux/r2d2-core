#[cfg(feature = "cuda")]
extern crate r2d2_cuda_core;

#[cfg(feature = "cuda")]
extern "C" {
    // Liaison dynamique avec le noyau R2D2
    fn chimera_w1a8_matmul(
        x_ptr: *const u8,
        w_ptr: *const u8,
        out_ptr: *mut f32,
        batch_seq: usize,
        hidden_dim: usize,
        intermediate_size: usize,
    ) -> i32;

    // ABI CUDA Native (Host-side) pour allouer de la VRAM sans dépendance tierce.
    // Cela prouve l'architecture "Zero-Bloat" : pas besoin de Candle ici,
    // on attaque directement le Driver Cuda C++.
    fn cudaMalloc(devPtr: *mut *mut std::ffi::c_void, size: usize) -> i32;
    fn cudaFree(devPtr: *mut std::ffi::c_void) -> i32;
    fn cudaMemcpy(
        dst: *mut std::ffi::c_void,
        src: *const std::ffi::c_void,
        count: usize,
        kind: i32,
    ) -> i32;
    fn cudaDeviceSynchronize() -> i32;
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_kernel_integrity_call() {
    // TDD GPU Phase 6: Vérification stricte de l'ABI et de l'intégrité du noyau PTX
    // Objectif : Prouver que NVCC a bien compilé le code, que le linker C++ l'a relié à Rust,
    // et qu'une passe forward peut s'exécuter dans la VRAM de la Nvidia sans Segfault.

    let in_f: usize = 2;
    let out_f: usize = 2;

    // Tenseurs d'entrée CPU (Signés int8)
    let h_x: Vec<i8> = vec![10, -5];
    // Poids ternaires (-1, 0, 1), aplati: w[out_f, in_f]
    // Row 0: [1, -1] -> 10*(1) + -5*(-1) = 15
    // Row 1: [0,  1] -> 10*(0) + -5*(1)  = -5
    let h_w: Vec<i8> = vec![1, -1, 0, 1];
    let mut h_out: Vec<f32> = vec![0.0, 0.0];

    let cuda_memcpy_h2d = 1;
    let cuda_memcpy_d2h = 2;

    unsafe {
        let mut d_x: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut d_w: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut d_out: *mut std::ffi::c_void = std::ptr::null_mut();

        let alloc_x = cudaMalloc(&mut d_x, in_f);
        let alloc_w = cudaMalloc(&mut d_w, in_f * out_f);
        let alloc_out = cudaMalloc(&mut d_out, out_f * 4); // f32 = 4 bytes

        assert_eq!(
            alloc_x, 0,
            "cudaMalloc a échoué. Le GPU est-il physiquement disponible dans le conteneur ?"
        );
        assert_eq!(alloc_w, 0, "cudaMalloc d_w a échoué.");
        assert_eq!(alloc_out, 0, "cudaMalloc d_out a échoué.");

        // Pousser les données CPU -> GPU
        cudaMemcpy(
            d_x,
            h_x.as_ptr() as *const std::ffi::c_void,
            in_f,
            cuda_memcpy_h2d,
        );
        cudaMemcpy(
            d_w,
            h_w.as_ptr() as *const std::ffi::c_void,
            in_f * out_f,
            cuda_memcpy_h2d,
        );

        // Exécution de l'Opération Tensorielle Custom (GPU Mathématique Réelle)
        let status_code = chimera_w1a8_matmul(
            d_x as *const u8,
            d_w as *const u8,
            d_out as *mut f32,
            1, // batch_seq = 1
            in_f,
            out_f,
        );

        let sync_status = cudaDeviceSynchronize();

        // Lire le résultat GPU -> CPU
        cudaMemcpy(
            h_out.as_mut_ptr() as *mut std::ffi::c_void,
            d_out as *const std::ffi::c_void,
            out_f * 4,
            cuda_memcpy_d2h,
        );

        // Nettoyage impitoyable de la mémoire (RAII manuel)
        cudaFree(d_x);
        cudaFree(d_w);
        cudaFree(d_out);

        assert_eq!(
            status_code, 0,
            "L'exécution du Kernel Custom CUDA a crashé."
        );
        assert_eq!(sync_status, 0, "Désynchronisation Cuda post-Kernel.");

        // Validation Intellectuelle de la Mathématique BitNet "Ternaire" (Preuve que le PTX calcule juste)
        assert_eq!(
            h_out[0], 15.0,
            "Le Kernel CUDA a mal calculé le produit scalaire ternaire [0] !"
        );
        assert_eq!(
            h_out[1], -5.0,
            "Le Kernel CUDA a mal calculé le produit scalaire ternaire [1] !"
        );
    }
}
