//! JUNO PATCH (LAC-9077): the vocoder's two hot layers, for the CPU.
//!
//! candle's CPU backend builds a convolution's input matrix an element at a
//! time, transposes the result the same way, and adds a per-channel bias or
//! scale through a strided iterator. On an Intel Mac, which speaks on the CPU,
//! that bookkeeping cost more than the arithmetic. Both functions here do the
//! same arithmetic on contiguous slices. GPU devices never reach them.

use candle_core::{DType, Result, Tensor};

/// Whether `x` can go through these paths.
pub(crate) fn applies(x: &Tensor) -> bool {
    x.device().is_cpu() && x.dtype() == DType::F32
}

/// A stride 1, ungrouped convolution's weight and bias as one matrix,
/// `[out, in * kernel + 1]`, the last column holding the bias. Made once, when
/// the layer is loaded.
pub(crate) fn conv1d_matrix(weight: &Tensor, bias: Option<&Tensor>) -> Result<Tensor> {
    let (outputs, inputs, kernel) = weight.dims3()?;
    let flat = weight.contiguous()?.reshape((outputs, inputs * kernel))?;
    let bias = match bias {
        Some(bias) => bias.contiguous()?.reshape((outputs, 1))?,
        None => Tensor::zeros((outputs, 1), weight.dtype(), weight.device())?,
    };
    Tensor::cat(&[&flat, &bias], 1)?.contiguous()
}

/// `conv1d` plus bias for `x` of shape `[1, in, length]`, stride 1, one group.
///
/// Row `i * kernel + j` of the input matrix is input channel `i` shifted by
/// tap `j`, so each row is one slice copy, and a last row of ones picks up the
/// bias. The product is already `[out, out_length]`, with nothing to
/// transpose.
pub(crate) fn conv1d(
    x: &Tensor,
    matrix: &Tensor,
    kernel: usize,
    padding: usize,
    dilation: usize,
) -> Result<Tensor> {
    let (batch, inputs, length) = x.dims3()?;
    let (outputs, columns) = matrix.dims2()?;
    let reach = dilation * kernel.saturating_sub(1);
    if batch != 1 || columns != inputs * kernel + 1 || length + 2 * padding <= reach {
        candle_core::bail!(
            "cpu conv1d: input {:?} does not fit kernel {kernel}, padding {padding}, \
             dilation {dilation} and matrix {:?}",
            x.dims(),
            matrix.dims()
        );
    }
    let out_length = length + 2 * padding - reach;
    let source = x.contiguous()?.flatten_all()?.to_vec1::<f32>()?;

    let mut shifted = vec![0f32; columns * out_length];
    let mut rows = shifted.chunks_exact_mut(out_length);
    for channel in source.chunks_exact(length) {
        for tap in 0..kernel {
            let Some(row) = rows.next() else { break };
            // Output step t reads input step t + offset - padding.
            let offset = tap * dilation;
            let first = padding.saturating_sub(offset).min(out_length);
            let end = (length + padding).saturating_sub(offset).min(out_length);
            if first < end {
                let from = first + offset - padding;
                row[first..end].copy_from_slice(&channel[from..from + (end - first)]);
            }
        }
    }
    if let Some(ones) = rows.next() {
        ones.fill(1.0);
    }

    let shifted = Tensor::from_vec(shifted, (columns, out_length), x.device())?;
    matrix.matmul(&shifted)?.reshape((1, outputs, out_length))
}

/// Instance normalisation followed by a per-channel scale and shift, in one
/// pass. `x` is `[batch, channels, length]`; `scale` and `shift` hold one
/// value per batch and channel.
pub(crate) fn normalize_then_modulate(
    x: &Tensor,
    eps: f32,
    scale: &[f32],
    shift: &[f32],
) -> Result<Tensor> {
    let (batch, channels, length) = x.dims3()?;
    let rows = batch * channels;
    if length == 0 || scale.len() != rows || shift.len() != rows {
        candle_core::bail!(
            "cpu instance norm: input {:?} with {} scales and {} shifts",
            x.dims(),
            scale.len(),
            shift.len()
        );
    }
    let mut values = x.contiguous()?.flatten_all()?.to_vec1::<f32>()?;
    for ((row, scale), shift) in values.chunks_exact_mut(length).zip(scale).zip(shift) {
        let mean = sum_of(row, |v| v) / length as f32;
        let variance = sum_of(row, |v| (v - mean) * (v - mean)) / length as f32;
        let gain = scale / (variance + eps).sqrt();
        for v in row.iter_mut() {
            *v = (*v - mean) * gain + shift;
        }
    }
    Tensor::from_vec(values, (batch, channels, length), x.device())
}

/// A sum in eight independent lanes, which the compiler can vectorise and
/// which loses less precision than one running total.
fn sum_of(values: &[f32], term: impl Fn(f32) -> f32) -> f32 {
    let mut lanes = [0f32; 8];
    let chunks = values.chunks_exact(8);
    let rest: f32 = chunks.remainder().iter().map(|v| term(*v)).sum();
    for chunk in chunks {
        for (lane, v) in lanes.iter_mut().zip(chunk) {
            *lane += term(*v);
        }
    }
    lanes.iter().sum::<f32>() + rest
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    fn largest_difference(a: &Tensor, b: &Tensor) -> f32 {
        assert_eq!(a.dims(), b.dims());
        (a - b)
            .unwrap()
            .abs()
            .unwrap()
            .flatten_all()
            .unwrap()
            .max(0)
            .unwrap()
            .to_scalar::<f32>()
            .unwrap()
    }

    #[test]
    fn conv1d_matches_candle() {
        let device = Device::Cpu;
        for (kernel, padding, dilation) in [
            (1, 0, 1),
            (3, 1, 1),
            (3, 3, 3),
            (7, 15, 5),
            (11, 5, 1),
            (5, 0, 1),
        ] {
            for length in [60, 61] {
                let x = Tensor::randn(0f32, 1.0, (1, 6, length), &device).unwrap();
                let weight = Tensor::randn(0f32, 0.2, (4, 6, kernel), &device).unwrap();
                let bias = Tensor::randn(0f32, 1.0, 4, &device).unwrap();

                let theirs = x.conv1d(&weight, padding, 1, dilation, 1).unwrap();
                let theirs = theirs
                    .broadcast_add(&bias.reshape((1, 4, 1)).unwrap())
                    .unwrap();
                let matrix = conv1d_matrix(&weight, Some(&bias)).unwrap();
                let ours = conv1d(&x, &matrix, kernel, padding, dilation).unwrap();

                let off = largest_difference(&ours, &theirs);
                assert!(off < 1e-4, "{kernel}/{padding}/{dilation}/{length}: {off}");
            }
        }
    }

    #[test]
    fn modulated_norm_matches_candle() {
        let device = Device::Cpu;
        let x = Tensor::randn(0.5f32, 2.0, (1, 5, 1003), &device).unwrap();
        let scale = Tensor::randn(1f32, 0.3, (1, 5, 1), &device).unwrap();
        let shift = Tensor::randn(0f32, 1.0, (1, 5, 1), &device).unwrap();

        let centered = x.broadcast_sub(&x.mean_keepdim(2).unwrap()).unwrap();
        let variance = centered.sqr().unwrap().mean_keepdim(2).unwrap();
        let deviation = (variance + 1e-5).unwrap().sqrt().unwrap();
        let theirs = centered
            .broadcast_div(&deviation)
            .unwrap()
            .broadcast_mul(&scale)
            .unwrap()
            .broadcast_add(&shift)
            .unwrap();
        let ours = normalize_then_modulate(
            &x,
            1e-5,
            &scale.flatten_all().unwrap().to_vec1().unwrap(),
            &shift.flatten_all().unwrap().to_vec1().unwrap(),
        )
        .unwrap();

        let off = largest_difference(&ours, &theirs);
        assert!(off < 1e-4, "{off}");
    }
}
