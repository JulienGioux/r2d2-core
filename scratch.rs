use candle_core::cuda_backend::cudarc::nvrtc::safe::compile_ptx_with_opts;
use candle_core::cuda_backend::cudarc::nvrtc::CompileOptions;
fn main() {
    let ptx = compile_ptx_with_opts("hello", CompileOptions::default()).unwrap();
    let ptx_str = ptx.to_str().unwrap();
}
