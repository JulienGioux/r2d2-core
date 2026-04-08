pub fn init() {}

#[cfg(not(feature = "cuda"))]
#[no_mangle]
pub extern "C" fn chimera_w1a8_matmul(
    _x_ptr: *const u8,
    _w_ptr: *const u8,
    _out_ptr: *mut f32,
    _batch_seq: usize,
    _hidden_dim: usize,
    _intermediate_size: usize,
) -> i32 {
    // Si cuda n'est pas activé, on retourne une erreur pour forcer le CPU fallback côté CustomOp
    -1
}
