#[cfg(feature = "cuda")]
use std::env;
#[cfg(feature = "cuda")]
use std::path::PathBuf;
#[cfg(feature = "cuda")]
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/kernels/chimera_cuda.cu");

    #[cfg(feature = "cuda")]
    {
        let out_dir = env::var("OUT_DIR").unwrap();
        let target_ptx = PathBuf::from(out_dir).join("chimera_cuda.ptx");

        // Utilisation explicite de nvcc pour générer du PTX PUR (Format Texte).
        // Cela résout le fameux bug de l'éditeur de liens 'rust-lld' sur Fedora,
        // car aucun objet statique (.a, .o) ou FFI C++ n'est exporté vers Rust !
        let status = Command::new("nvcc")
            .arg("--ptx")
            .arg("-O3")
            .arg("-arch=native")
            .arg("src/kernels/chimera_cuda.cu")
            .arg("-o")
            .arg(&target_ptx)
            .status()
            .expect("Erreur fatale: 'nvcc' non trouvé dans le PATH ou erreur d'exécution.");

        if !status.success() {
            panic!("La compilation NVCC a échoué (Status {:?})", status.code());
        }
    }
}
