fn main() {
    println!("cargo:rerun-if-changed=src/kernels/chimera_cuda.cu");

    if std::env::var("CARGO_FEATURE_CUDA").is_ok() {
        // L'Architecte cible explicitement nvcc pour contourner les limitations
        // des vieux drivers nvidia et lier notre logique MatMul-free en GPU.
        cc::Build::new()
            .cuda(true)
            // Compilation "Fatbin" Universelle pour couvrir toutes les cartes NVIDIA modernes
            .flag("-gencode")
            .flag("arch=compute_86,code=sm_86") // Ampere (RTX 3000)
            .flag("-gencode")
            .flag("arch=compute_89,code=sm_89") // Ada (RTX 4000)
            // Fallback ultime : Code intermédiaire compilé à la volée par le Driver pour toute autre carte
            .flag("-gencode")
            .flag("arch=compute_86,code=compute_86")
            .flag("-O3")
            .flag("-use_fast_math")
            .file("src/kernels/chimera_cuda.cu")
            .compile("chimera_cuda_lib");

        // Directive pour forcer le linkage sur la runtime CUDA indispensable à cudaMalloc/cudaFree
        println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
        println!("cargo:rustc-link-search=native=/usr/local/cuda/targets/x86_64-linux/lib");
        println!("cargo:rustc-link-lib=dylib=cudart");
        println!("cargo:rustc-link-lib=dylib=cuda");
    }
}
