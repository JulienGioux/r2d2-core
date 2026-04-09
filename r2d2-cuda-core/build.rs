fn main() {
    println!("cargo:rerun-if-changed=src/kernels/chimera_cuda.cu");

    // Suite à l'audit "Sovereign Shield", l'édition des liens (LD) de la crate cc
    // a été bannie pour éviter les crashs FFI.
    // Le code Cuda sera compilé en PTX ou chargé dynamiquement au runtime.
}
