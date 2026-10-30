//! Everything Juno knows about the Parakeet models without being able to run
//! them.
//!
//! Parakeet's loader (`parakeet-rs`, and under it `ort-sys`) ships a prebuilt
//! ONNX Runtime for `aarch64-apple-darwin` and nothing else, so
//! `engine_parakeet` is compiled on Apple Silicon only. Everything
//! that never touches the loader lives here and is compiled on every
//! architecture: the download manifests, what is on disk, and whether this
//! build can run any of it.
//!
//! An Intel build needs all of that to answer the UI honestly. It just never
//! constructs an engine: see [`PARAKEET_SUPPORTED`].
//!
//! ## Two Parakeet models
//!
//! Juno has shipped two. They are the same 0.6B FastConformer encoder with
//! different heads, and the difference is what a person reads back:
//!
//! - **CTC** writes everything in lowercase with no punctuation. It was the
//!   first one wired up, and people who downloaded it still have it.
//! - **TDT** (v2, English) writes capitals and punctuation itself and has the
//!   lower word error rate (6.05 against CTC's 7.7 on the Open ASR sets). It
//!   is the one the Models pane recommends.
//!
//! Each lives in its own directory under `<app data>/models/`, named by
//! [`ParakeetVariant::dir_leaf`], and that name is how every path-taking
//! function here knows which manifest applies. One engine loads either; see
//! `engine_parakeet`.

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

/// One file a Parakeet loader needs on disk.
#[derive(Debug, Clone, Copy)]
pub struct ParakeetFile {
    /// File name inside the model directory (what the loader looks for).
    pub name: &'static str,
    /// Path inside the HuggingFace repo.
    pub remote_path: &'static str,
    /// Exact size of the file at the variant's pinned revision.
    pub bytes: u64,
}

/// Which Parakeet model a directory holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParakeetVariant {
    /// Parakeet CTC 0.6B: lowercase, no punctuation. The original.
    Ctc,
    /// Parakeet TDT 0.6B v2 (English): capitals and punctuation built in.
    Tdt,
}

/// The int8 export of Parakeet CTC 0.6B: 612 MB on disk, the same weight class
/// as Whisper large-v3-turbo q5_0, and the one parakeet-rs documents as the
/// quantized variant. The fp32 export (`model.onnx` + 2.4 GB of weights) loads
/// too, but it is four times the download for the same word error rate class.
///
/// `parakeet-rs` looks for `model.onnx`, then `model_fp16.onnx`, then
/// `model_int8.onnx` in the directory; with only the int8 files present it
/// picks the int8 graph, which loads its weights from `model_int8.onnx_data`
/// next to it.
const CTC_FILES: &[ParakeetFile] = &[
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

/// The int8 export of Parakeet TDT 0.6B v2: 661 MB on disk. Two graphs (the
/// encoder, and the prediction network joined with the joint network) plus
/// the SentencePiece vocabulary; the blank token is the vocabulary's last
/// entry, which is what `parakeet-rs` assumes.
///
/// The fp32 encoder (`encoder-model.onnx` + 2.4 GB of weights) is left out
/// for the same reason as CTC's. `parakeet-rs` tries `encoder-model.onnx`
/// first and falls through to `encoder-model.int8.onnx`, so a directory
/// holding only the int8 files loads the int8 graphs.
const TDT_FILES: &[ParakeetFile] = &[
    ParakeetFile {
        name: "encoder-model.int8.onnx",
        remote_path: "encoder-model.int8.onnx",
        bytes: 652_184_014,
    },
    ParakeetFile {
        name: "decoder_joint-model.int8.onnx",
        remote_path: "decoder_joint-model.int8.onnx",
        bytes: 8_998_286,
    },
    ParakeetFile {
        name: "vocab.txt",
        remote_path: "vocab.txt",
        bytes: 9_384,
    },
];

impl ParakeetVariant {
    pub const ALL: [ParakeetVariant; 2] = [ParakeetVariant::Ctc, ParakeetVariant::Tdt];

    /// The directory name under `<app data>/models/` this model lives in.
    pub fn dir_leaf(self) -> &'static str {
        match self {
            ParakeetVariant::Ctc => "parakeet-ctc",
            ParakeetVariant::Tdt => "parakeet-tdt",
        }
    }

    /// The variant a model directory holds, from its name. `None` for a
    /// directory that is not one of ours.
    pub fn for_leaf(leaf: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.dir_leaf() == leaf)
    }

    /// The variant a model directory holds, from its path. A path that does
    /// not end in one of our directory names is read as CTC, the layout every
    /// install before TDT had, so an old config value keeps meaning what it
    /// meant.
    pub fn for_dir(dir: &Path) -> Self {
        dir.file_name()
            .and_then(|n| n.to_str())
            .and_then(Self::for_leaf)
            .unwrap_or(ParakeetVariant::Ctc)
    }

    /// The HuggingFace repo the ONNX export is fetched from.
    pub fn hf_repo(self) -> &'static str {
        match self {
            ParakeetVariant::Ctc => "onnx-community/parakeet-ctc-0.6b-ONNX",
            ParakeetVariant::Tdt => "istupakov/parakeet-tdt-0.6b-v2-onnx",
        }
    }

    /// Pinned revision of that repo. The byte counts in the manifest are
    /// verified after download, so the download must always fetch the exact
    /// commit they were measured against, never a moving `main`.
    pub fn hf_revision(self) -> &'static str {
        match self {
            ParakeetVariant::Ctc => "7df2cab7aed886b8b7f80d68a8214007e4847601",
            ParakeetVariant::Tdt => "0bbb45a3365852604aef28b538a8f066f4ccaa85",
        }
    }

    /// Every file the loader needs on disk.
    pub fn files(self) -> &'static [ParakeetFile] {
        match self {
            ParakeetVariant::Ctc => CTC_FILES,
            ParakeetVariant::Tdt => TDT_FILES,
        }
    }

    /// Total download size of [`Self::files`].
    pub fn total_bytes(self) -> u64 {
        self.files().iter().map(|f| f.bytes).sum()
    }

    /// Download URL for one manifest entry at the pinned revision.
    pub fn file_url(self, file: &ParakeetFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.hf_repo(),
            self.hf_revision(),
            file.remote_path
        )
    }

    /// Names of the manifest files that are not in `model_dir` (or are the
    /// wrong size, which means a partial or foreign file the loader would
    /// choke on).
    pub fn missing_files(self, model_dir: &Path) -> Vec<&'static str> {
        self.files()
            .iter()
            .filter(|f| {
                std::fs::metadata(model_dir.join(f.name))
                    .map(|m| m.len() != f.bytes)
                    .unwrap_or(true)
            })
            .map(|f| f.name)
            .collect()
    }
}

/// Names of the manifest files that are not in `model_dir`, for whichever
/// Parakeet model that directory is named for.
pub fn missing_parakeet_files(model_dir: &Path) -> Vec<&'static str> {
    ParakeetVariant::for_dir(model_dir).missing_files(model_dir)
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

/// On-disk state of a Parakeet model directory, plus whether this build
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
        let variant = ParakeetVariant::for_dir(model_dir);
        let missing = variant.missing_files(model_dir);
        let present = variant
            .files()
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
            total_bytes: variant.total_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_manifest_is_the_int8_export_the_loader_finds() {
        let names: Vec<_> = CTC_FILES.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            ["model_int8.onnx", "model_int8.onnx_data", "tokenizer.json"]
        );
        // 612 MB, the number the Models pane shows.
        assert_eq!(ParakeetVariant::Ctc.total_bytes(), 612_689_838);
        let url = ParakeetVariant::Ctc.file_url(&CTC_FILES[0]);
        assert!(url.contains(ParakeetVariant::Ctc.hf_revision()));
    }

    /// The TDT loader in parakeet-rs looks for these names and nothing else
    /// we ship: `encoder-model.int8.onnx` is third in its encoder candidate
    /// list (after the fp32 names we do not download) and
    /// `decoder_joint-model.int8.onnx` second in its decoder list, with
    /// `vocab.txt` required by name.
    #[test]
    fn tdt_manifest_is_the_int8_export_the_loader_finds() {
        let names: Vec<_> = TDT_FILES.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "encoder-model.int8.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt"
            ]
        );
        // 661 MB, the number the Models pane shows.
        assert_eq!(ParakeetVariant::Tdt.total_bytes(), 661_191_684);
        let url = ParakeetVariant::Tdt.file_url(&TDT_FILES[0]);
        assert!(url.contains("parakeet-tdt-0.6b-v2-onnx"));
        assert!(url.contains(ParakeetVariant::Tdt.hf_revision()));
    }

    /// The directory name is the only thing that says which model a
    /// directory holds, and an old config value ("models/parakeet-ctc", or
    /// anything older) has to keep meaning CTC.
    #[test]
    fn the_directory_name_picks_the_manifest() {
        assert_eq!(
            ParakeetVariant::for_dir(Path::new("/x/models/parakeet-tdt")),
            ParakeetVariant::Tdt
        );
        assert_eq!(
            ParakeetVariant::for_dir(Path::new("/x/models/parakeet-ctc")),
            ParakeetVariant::Ctc
        );
        assert_eq!(
            ParakeetVariant::for_dir(Path::new("/x/models/parakeet")),
            ParakeetVariant::Ctc
        );
        assert_eq!(
            ParakeetVariant::for_leaf("parakeet-tdt"),
            Some(ParakeetVariant::Tdt)
        );
        assert_eq!(ParakeetVariant::for_leaf("whisper"), None);
        for v in ParakeetVariant::ALL {
            assert_eq!(ParakeetVariant::for_leaf(v.dir_leaf()), Some(v));
        }
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

    /// A TDT directory is judged by the TDT manifest: CTC files in it count
    /// for nothing, and the status reports the TDT download size.
    #[test]
    fn a_tdt_directory_is_checked_against_the_tdt_manifest() {
        let dir = std::env::temp_dir()
            .join(format!("juno-parakeet-tdt-test-{}", std::process::id()))
            .join("parakeet-tdt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in CTC_FILES {
            std::fs::write(dir.join(f.name), b"ctc").unwrap();
        }
        let missing = missing_parakeet_files(&dir);
        assert_eq!(
            missing,
            [
                "encoder-model.int8.onnx",
                "decoder_joint-model.int8.onnx",
                "vocab.txt"
            ]
        );
        assert_eq!(
            ParakeetModelStatus::check(&dir).total_bytes,
            ParakeetVariant::Tdt.total_bytes()
        );
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
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
