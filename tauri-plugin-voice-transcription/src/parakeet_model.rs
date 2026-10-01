//! What Parakeet's model is made of, and whether it is on disk.
//!
//! Deliberately separate from `engine_parakeet`, which holds the loader. This
//! module touches only the filesystem and so compiles on every architecture;
//! the loader reaches ONNX Runtime through `parakeet-rs` -> `ort-sys`, whose
//! prebuilt distribution has no `x86_64-apple-darwin` entry, so it is compiled
//! on Apple Silicon only.
//!
//! The split is along that dependency, not along the feature. An Intel Mac
//! still needs to answer "is Parakeet downloaded?" — the download manifest is
//! what `commands/stt_models.rs` builds the Models pane from, and that pane
//! reports on a machine that cannot run the engine just as well as one that
//! can.

use std::path::Path;

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

/// Whether every required model file is present (and complete) without loading
/// any of them.
pub fn parakeet_model_files_present(model_dir: &Path) -> bool {
    missing_parakeet_files(model_dir).is_empty()
}

/// On-disk state of the Parakeet model directory.
#[derive(Debug, serde::Serialize)]
pub struct ParakeetModelStatus {
    pub downloaded: bool,
    pub model_dir: String,
    pub files_present: Vec<String>,
    pub files_missing: Vec<String>,
    /// Bytes the complete model occupies once downloaded.
    pub total_bytes: u64,
    /// Whether this build can run Parakeet at all. False on Intel: the engine
    /// is not compiled there, so a downloaded model would still not load.
    /// Reported separately from `downloaded` so the two reasons a person
    /// cannot use Parakeet never get confused for each other.
    pub supported: bool,
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
            downloaded: missing.is_empty(),
            model_dir: model_dir.to_string_lossy().into_owned(),
            files_present: present,
            files_missing: missing.into_iter().map(str::to_string).collect(),
            total_bytes: parakeet_total_bytes(),
            supported: crate::engine::parakeet_is_supported(),
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
        std::fs::create_dir_all(&dir).expect("temp dir");
        assert_eq!(missing_parakeet_files(&dir).len(), 3);
        assert!(!parakeet_model_files_present(&dir));

        std::fs::write(dir.join("tokenizer.json"), b"{}").expect("tokenizer");
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

    /// The manifest answers the same way on an Intel Mac; only `supported`
    /// changes. A status call is how the Models pane explains itself, and it
    /// must not start erroring on the architecture that cannot run the engine.
    #[test]
    fn status_reports_support_separately_from_download_state() {
        let dir = std::env::temp_dir().join(format!("juno-parakeet-sup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        let status = ParakeetModelStatus::check(&dir);
        assert!(!status.downloaded, "nothing was written");
        assert_eq!(status.supported, cfg!(target_arch = "aarch64"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
