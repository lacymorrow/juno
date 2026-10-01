//! Everything Juno knows about the Parakeet model without being able to run it.
//!
//! Parakeet's loader (`parakeet-rs`, and under it `ort-sys`) ships a prebuilt
//! ONNX Runtime for `aarch64-apple-darwin` and nothing else, so
//! `engine_parakeet` is compiled on Apple Silicon only. Everything
//! that never touches the loader lives here and is compiled on every
//! architecture: the download manifest, what is on disk, and whether this
//! build can run any of it.
//!
//! An Intel build needs all of that to answer the UI honestly. It just never
//! constructs an engine: see [`PARAKEET_SUPPORTED`].

use std::path::Path;

/// Whether this build can actually run Parakeet.
///
/// The single source of truth for the architecture gate. It is derived from
/// the same `cfg` that decides whether `parakeet-rs` is a dependency at all
/// (see this crate's Cargo.toml), so it cannot drift from what links.
pub const PARAKEET_SUPPORTED: bool = cfg!(target_arch = "aarch64");

/// What to tell a person on a Mac that cannot run Parakeet.
///
/// Phrased for a settings pane, not a log: it says what the machine can do,
/// not what the build failed to include.
pub const PARAKEET_UNSUPPORTED_REASON: &str =
    "Parakeet needs an Apple Silicon Mac. Its speech engine has no Intel build, \
     so dictation on this Mac uses Whisper.";

/// Why Parakeet is unavailable, or `None` when it is available.
pub fn parakeet_unsupported_reason() -> Option<&'static str> {
    (!PARAKEET_SUPPORTED).then_some(PARAKEET_UNSUPPORTED_REASON)
}

/// The HuggingFace repo the Parakeet CTC ONNX export is fetched from.
pub const PARAKEET_HF_REPO: &str = "onnx-community/parakeet-ctc-0.6b-ONNX";

/// Pinned revision of that repo. The byte counts in `PARAKEET_MODEL_FILES` are
/// verified after download, so the download must always fetch the exact commit
/// they were measured against, never a moving `main`.
pub const PARAKEET_HF_REVISION: &str = "7df2cab7aed886b8b7f80d68a8214007e4847601";

/// One file `Parakeet::from_pretrained(model_dir)` needs on disk.
#[derive(Debug, Clone, Copy)]
pub struct ParakeetFile {
    /// File name inside the model directory (what the loader looks for).
    pub name: &'static str,
    /// Path inside the HuggingFace repo.
    pub remote_path: &'static str,
    /// Exact size of the file at `PARAKEET_HF_REVISION`.
    pub bytes: u64,
}

/// The int8 export of Parakeet CTC 0.6B: 612 MB on disk, the same weight class
/// as Whisper large-v3-turbo q5_0, and the one parakeet-rs documents as the
/// quantized variant. The fp32 export (`model.onnx` + 2.4 GB of weights) loads
/// too, but it is four times the download for the same word error rate class,
/// which is the wrong default for something that fetches itself on first run.
///
/// `parakeet-rs` looks for `model.onnx`, then `model_fp16.onnx`, then
/// `model_int8.onnx` in the directory; with only the int8 files present it
/// picks the int8 graph, which loads its weights from `model_int8.onnx_data`
/// next to it.
pub const PARAKEET_MODEL_FILES: &[ParakeetFile] = &[
    ParakeetFile {
        name: "model_int8.onnx",
        remote_path: "onnx/model_int8.onnx",
        bytes: 1_303_007,
    },
    ParakeetFile {
        name: "model_int8.onnx_data",
        remote_path: "onnx/model_int8.onnx_data",
        bytes: 610_974_468,
    },
    ParakeetFile {
        name: "tokenizer.json",
        remote_path: "tokenizer.json",
        bytes: 412_363,
    },
];

/// Total download size of `PARAKEET_MODEL_FILES`.
pub fn parakeet_total_bytes() -> u64 {
    PARAKEET_MODEL_FILES.iter().map(|f| f.bytes).sum()
}

/// Download URL for one manifest entry at the pinned revision.
pub fn parakeet_file_url(file: &ParakeetFile) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        PARAKEET_HF_REPO, PARAKEET_HF_REVISION, file.remote_path
    )
}

/// Names of the manifest files that are not in `model_dir` (or are the wrong
/// size, which means a partial or foreign file the loader would choke on).
pub fn missing_parakeet_files(model_dir: &Path) -> Vec<&'static str> {
    PARAKEET_MODEL_FILES
        .iter()
        .filter(|f| {
            std::fs::metadata(model_dir.join(f.name))
                .map(|m| m.len() != f.bytes)
                .unwrap_or(true)
        })
        .map(|f| f.name)
        .collect()
}

/// Whether every required model file is present (and complete) without
/// loading any of them. Says nothing about whether this build can run them.
pub fn model_files_present(model_dir: &Path) -> bool {
    missing_parakeet_files(model_dir).is_empty()
}

/// Parakeet can be loaded right now: this build supports it *and* every model
/// file is on disk.
///
/// This is the question callers almost always mean. Asking only about the
/// files would boot an engine that does not exist in this binary.
pub fn parakeet_ready(model_dir: &Path) -> bool {
    PARAKEET_SUPPORTED && model_files_present(model_dir)
}

/// On-disk state of the Parakeet model directory, plus whether this build
/// could use it.
///
/// `downloaded` is literal: the files are there and complete. It can be true
/// on a Mac that cannot run them (an Intel Mac restored from an Apple Silicon
/// backup), so `supported` is the field that decides whether Parakeet is
/// offered, and `unsupported_reason` is what to show instead.
#[derive(Debug, serde::Serialize)]
pub struct ParakeetModelStatus {
    /// This build can run Parakeet.
    pub supported: bool,
    /// Why not, when `supported` is false.
    pub unsupported_reason: Option<String>,
    pub downloaded: bool,
    pub model_dir: String,
    pub files_present: Vec<String>,
    pub files_missing: Vec<String>,
    /// Bytes the complete model occupies once downloaded.
    pub total_bytes: u64,
}

impl ParakeetModelStatus {
    pub fn check(model_dir: &Path) -> Self {
        let missing = missing_parakeet_files(model_dir);
        let present = PARAKEET_MODEL_FILES
            .iter()
            .map(|f| f.name)
            .filter(|name| !missing.contains(name))
            .map(str::to_string)
            .collect();

        Self {
            supported: PARAKEET_SUPPORTED,
            unsupported_reason: parakeet_unsupported_reason().map(str::to_string),
            downloaded: missing.is_empty(),
            model_dir: model_dir.to_string_lossy().into_owned(),
            files_present: present,
            files_missing: missing.into_iter().map(str::to_string).collect(),
            total_bytes: parakeet_total_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_the_int8_export_the_loader_finds() {
        let names: Vec<_> = PARAKEET_MODEL_FILES.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            ["model_int8.onnx", "model_int8.onnx_data", "tokenizer.json"]
        );
        // 612 MB, the number the Models pane shows.
        assert_eq!(parakeet_total_bytes(), 612_689_838);
        assert!(parakeet_file_url(&PARAKEET_MODEL_FILES[0]).contains(PARAKEET_HF_REVISION));
    }

    #[test]
    fn a_partial_file_counts_as_missing() {
        let dir = std::env::temp_dir().join(format!("juno-parakeet-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(missing_parakeet_files(&dir).len(), 3);
        assert!(!model_files_present(&dir));

        std::fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        let missing = missing_parakeet_files(&dir);
        assert!(
            missing.contains(&"tokenizer.json"),
            "wrong size is not downloaded"
        );

        let status = ParakeetModelStatus::check(&dir);
        assert!(!status.downloaded);
        assert_eq!(status.files_missing.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The arch gate reports itself, and says why when it is closed. Without
    /// a reason the UI has nothing to show in place of the option, which is
    /// how a disabled control becomes a mystery.
    #[test]
    fn an_unsupported_build_says_why_and_a_supported_one_does_not() {
        let status = ParakeetModelStatus::check(Path::new("/nowhere"));
        if PARAKEET_SUPPORTED {
            assert!(parakeet_unsupported_reason().is_none());
            assert!(status.supported);
            assert!(status.unsupported_reason.is_none());
        } else {
            assert!(parakeet_unsupported_reason().is_some_and(|r| r.contains("Apple Silicon")));
            assert!(!status.supported);
            assert!(status.unsupported_reason.is_some());
        }
    }

    /// Files on disk are not enough on a Mac that cannot run them. This is the
    /// check that keeps an Intel build from booting an engine it does not
    /// contain, and it is why callers ask `parakeet_ready` rather than
    /// `model_files_present`.
    #[test]
    fn readiness_needs_support_as_well_as_files() {
        let dir = std::env::temp_dir().join(format!("juno-parakeet-ready-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Nothing downloaded: never ready, on any architecture.
        assert!(!model_files_present(&dir));
        assert!(!parakeet_ready(&dir));

        // And where the files are present, readiness is exactly the arch gate.
        // Written as the implication rather than by staging 612 MB of files:
        // the half that matters here is that support can veto them.
        if !PARAKEET_SUPPORTED {
            assert!(
                !parakeet_ready(&dir),
                "an unsupported build is never ready, whatever is on disk"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
