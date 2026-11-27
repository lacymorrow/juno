# any-tts 0.1.3, patched

Upstream: https://crates.io/crates/any-tts/0.1.3 (MIT OR Apache-2.0). The
published examples, tests and dev-dependencies are left out.

Every change is marked `JUNO PATCH` (LAC-9077). All of them are about speaking
on a CPU, which is what Intel Macs do. GPU devices keep the upstream paths,
except for the LSTM change, which helps both.

| Where | Change |
|---|---|
| `layers/lstm.rs` | The weights are transposed once per pass, not on every timestep |
| `layers/cpu.rs`, `Conv1d` | CPU convolution builds its input matrix from slice copies and takes the bias in the same multiply |
| `layers/cpu.rs`, `AdaIn1d` | CPU instance norm, scale and shift in one pass |
| `ConvTranspose1d` | On the CPU a padded transposed convolution runs unpadded and is trimmed, because candle 0.10.2 has no fast path for padding |

## Measured

Kokoro on an i7-9750H, F32, mean seconds of work per second of speech, four
sentences of 1.4 to 5.5 s:

| Build | RTF |
|---|---|
| Upstream | 1.96 to 2.50 |
| Patched | 0.92 to 1.03 |
| Patched, with candle's `accelerate` (what Intel Macs get) | 0.78 to 0.97 |

`accelerate` alone, without these patches, was worth about 10%.

## Testing

This crate is outside the workspace, so Juno's CI compiles it and does not run
its tests. `layers::cpu::tests` compares both CPU functions with candle's own
operations. After touching `layers/`, run on a machine allowed to compile:

```bash
cargo test --release --lib --no-default-features --features kokoro,download layers::
```

(add an empty `[workspace]` table to a copy of the crate first).

Kokoro adds random noise to every sentence, so two renders of the same text
never match sample for sample. Do not compare whole waveforms.

Remove a patch once upstream candle or any-tts does the same job.
