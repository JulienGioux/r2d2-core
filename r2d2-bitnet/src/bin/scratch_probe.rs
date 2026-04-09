use candle_core::{Device, Tensor};

fn main() -> candle_core::Result<()> {
    let dev = Device::Cpu;
    let x = Tensor::new(&[[1.0f32, 2.0], [3.0, 4.0], [5.0, 6.0]], &dev)?;
    let idx = Tensor::new(&[0u32, 2u32], &dev)?;

    let selected = x.index_select(&idx, 0)?;
    println!("Selected: {}", selected);

    let mut out = Tensor::zeros((3, 2), candle_core::DType::F32, &dev)?;
    out = out.index_add(&idx, &selected, 0)?;

    println!("Out: {}", out);

    Ok(())
}
