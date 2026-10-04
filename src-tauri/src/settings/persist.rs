//! # Settings files that cannot be damaged
//!
//! Juno's settings, and every other store file it keeps, used to be written
//! the way `tauri-plugin-store` writes them: `fs::write`, which truncates the
//! file and then fills it. A crash, a force quit or a full disk between the two
//! left half a JSON file. On the next launch the plugin swallowed the parse
//! error and started empty, the first save then wrote over the remains, and
//! the person found every setting back at its default with nothing said.
//!
//! Three things stop that, and this module holds the Juno side of all three:
//!
//! 1. **Writes are atomic.** The vendored `tauri-plugin-store` (see
//!    `vendor/tauri-plugin-store`) writes a temp file in the same folder,
//!    fsyncs it and renames it over the original, so the file on disk is the
//!    old one or the new one. [`atomic_write`] is the same routine, tested
//!    here. [`one_writer`] gives the settings manager's saves one turn each.
//! 2. **Reads are tolerant.** [`lenient`] reads a section field by field: an
//!    unknown field is ignored, a missing one takes its default, and one field
//!    of the wrong type falls back on its own instead of resetting the
//!    section.
//! 3. **A file that still cannot be read is replaced, silently.** Before
//!    anything opens a store, [`guard_plugin`] checks each one. A file that is
//!    not a JSON object is replaced with a clean, empty store, so it starts
//!    from defaults. Nothing is kept and nothing is shown to the person: an
//!    internal failure is Juno's to absorb, never theirs to read about. It is
//!    logged at warn level.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};
use tracing::{error, warn};

use crate::constants::settings::{store_files, SETTINGS_STORE_FILE};

/// The store files checked at launch, before anything opens them.
pub const GUARDED_FILES: &[&str] = &[
    SETTINGS_STORE_FILE,
    store_files::BAR_POSITION,
    store_files::MEMORY,
    store_files::SCHEDULED_AUTOMATIONS,
    store_files::ONBOARDING_ANALYTICS,
    crate::conversation_history::INDEX_FILE,
];

/// A temp file older than this was left by a crash, not by a save in flight.
const STALE_TEMP_AGE: Duration = Duration::from_secs(60);

// --- 1. Atomic writes, one writer ---

/// Replace `path` so that it holds either its old bytes or `bytes`, never a
/// mix and never a truncated file.
///
/// Kept identical to `write_atomically` in `vendor/tauri-plugin-store`, which
/// is what every store save runs; `vendored_store_saves_atomically` pins that.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_with(path, bytes, || Ok(()))
}

/// [`atomic_write`] with a hook that runs after the temp file is on disk and
/// before the rename, which is where a test simulates a crash.
fn atomic_write_with(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let dir = parent_dir(path);
    let tmp = dir.join(format!(
        ".{}.{}-{}.tmp",
        file_name(path),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));

    let result = replace_with_temp(&tmp, path, dir, bytes, before_rename);
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn replace_with_temp(
    tmp: &Path,
    path: &Path,
    dir: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    before_rename()?;
    fs::rename(tmp, path)?;
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

static WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Run one settings write with nobody else writing.
///
/// The settings manager puts its entries in the store and saves inside this,
/// so a whole-settings write is one unit and two saves never interleave.
pub async fn one_writer<R>(write: impl FnOnce() -> R) -> R {
    let _turn = WRITE_LOCK.lock().await;
    write()
}

// --- 2. Tolerant reads ---

/// Read `stored` as a `T`, keeping every field that can be kept.
///
/// A value that parses is returned as is, so nothing changes for a healthy
/// file. Otherwise this starts from `T::default()` and lays the stored fields
/// over it one at a time, descending into nested objects, and keeps each one
/// that still leaves a valid `T`. Unknown fields ride along harmlessly; a
/// field that does not fit is dropped and logged by name (never by value,
/// since a value can be an API key).
pub fn lenient<T>(section: &str, stored: &Value) -> T
where
    T: DeserializeOwned + Serialize + Default,
{
    if let Ok(parsed) = serde_json::from_value::<T>(stored.clone()) {
        return parsed;
    }
    let fits = |candidate: &Value| serde_json::from_value::<T>(candidate.clone()).is_ok();
    let mut accepted = match serde_json::to_value(T::default()) {
        Ok(base) if fits(&base) => base,
        _ => {
            warn!("[Settings] {}: could not be read; using defaults", section);
            return T::default();
        }
    };
    let Value::Object(fields) = stored else {
        warn!(
            "[Settings] {}: is not a set of fields; using defaults",
            section
        );
        return T::default();
    };

    let mut dropped = Vec::new();
    salvage(&mut accepted, &mut Vec::new(), fields, &fits, &mut dropped);
    if !dropped.is_empty() {
        warn!(
            "[Settings] {}: could not read {}; those use their defaults",
            section,
            dropped.join(", ")
        );
    }
    serde_json::from_value(accepted).unwrap_or_default()
}

fn salvage(
    accepted: &mut Value,
    path: &mut Vec<String>,
    stored: &Map<String, Value>,
    fits: &dyn Fn(&Value) -> bool,
    dropped: &mut Vec<String>,
) {
    for (key, value) in stored {
        let mut candidate = accepted.clone();
        let Some(target) = object_at(&mut candidate, path) else {
            return;
        };
        let default_is_object = matches!(target.get(key), Some(Value::Object(_)));
        target.insert(key.clone(), value.clone());
        if fits(&candidate) {
            *accepted = candidate;
            continue;
        }
        match value {
            Value::Object(inner) if default_is_object => {
                path.push(key.clone());
                salvage(accepted, path, inner, fits, dropped);
                path.pop();
            }
            _ => {
                let mut name = path.join(".");
                if !name.is_empty() {
                    name.push('.');
                }
                name.push_str(key);
                dropped.push(name);
            }
        }
    }
}

fn object_at<'a>(value: &'a mut Value, path: &[String]) -> Option<&'a mut Map<String, Value>> {
    let mut current = value;
    for key in path {
        current = current.as_object_mut()?.get_mut(key)?;
    }
    current.as_object_mut()
}

// --- 3. Replace what cannot be read ---

/// Check one store file before it is opened.
///
/// A missing file, or one that reads as a JSON object, is left alone and
/// returns `false`. A file that cannot be read for another reason (a
/// permission, say) is also left alone: it is not damaged. Anything else is
/// replaced, atomically, with an empty valid store and returns `true`, so the
/// plugin never starts empty over a file it then overwrites.
pub fn recover_if_damaged(path: &Path) -> bool {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return false,
        Err(e) => {
            warn!("[Settings] Could not read {}: {}", path.display(), e);
            return false;
        }
    };
    if serde_json::from_slice::<Map<String, Value>>(&bytes).is_ok() {
        return false;
    }
    match atomic_write(path, b"{}") {
        Ok(()) => warn!(
            "[Settings] {} could not be read; replaced it and started from defaults",
            path.display()
        ),
        Err(e) => error!(
            "[Settings] Could not replace unreadable {}: {}",
            path.display(),
            e
        ),
    }
    true
}

/// Remove temp files a crash left behind for `name`.
fn sweep_stale_temps(dir: &Path, name: &str) {
    let prefix = format!(".{name}.");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        let file = file_name(&path);
        if !(file.starts_with(&prefix) && file.ends_with(".tmp")) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if stale {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Check every guarded file in `dir`. Returns the names that were replaced.
pub fn guard_dir(dir: &Path) -> Vec<&'static str> {
    let mut replaced = Vec::new();
    for name in GUARDED_FILES {
        sweep_stale_temps(dir, name);
        if recover_if_damaged(&dir.join(name)) {
            replaced.push(*name);
        }
    }
    replaced
}

/// A plugin whose only job is to run [`guard_dir`] before any store opens.
///
/// Register it before `tauri_plugin_store` and the voice plugin: plugin setup
/// hooks run in registration order, and the voice plugin reads the settings
/// store from its own hook.
pub fn guard_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("juno-settings-guard")
        .setup(|app, _api| {
            use tauri::Manager;
            match app.path().app_data_dir() {
                Ok(dir) => {
                    guard_dir(&dir);
                }
                Err(e) => warn!("[Settings] No app data folder to check: {}", e),
            }
            Ok(())
        })
        .build()
}

// --- Paths ---

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::Arc;

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).expect("file exists")).expect("valid json")
    }

    fn leftover_temps(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .expect("dir")
            .filter_map(|e| e.ok())
            .map(|e| file_name(&e.path()))
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn a_crash_before_the_rename_leaves_the_old_file_whole() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(SETTINGS_STORE_FILE);
        atomic_write(&path, br#"{"version":"old"}"#).expect("first write");

        let crashed = atomic_write_with(&path, br#"{"version":"new"}"#, || {
            Err(io::Error::other("simulated crash"))
        });

        assert!(crashed.is_err());
        assert_eq!(read_json(&path), serde_json::json!({"version": "old"}));
        assert!(leftover_temps(dir.path()).is_empty());

        atomic_write(&path, br#"{"version":"new"}"#).expect("second write");
        assert_eq!(read_json(&path), serde_json::json!({"version": "new"}));
    }

    #[test]
    fn vendored_store_saves_atomically() {
        // The plugin's save is the write that matters, and it lives outside
        // this crate. Pin it to the same routine so it cannot drift back to
        // `fs::write`.
        let source = include_str!("../../../vendor/tauri-plugin-store/src/store.rs");
        assert!(source.contains("write_atomically(&self.path, &bytes)?;"));
        assert!(source.contains("fs::rename(tmp, path)?;"));
        assert!(source.contains("file.sync_all()?;"));
        assert!(!source.contains("fs::write(&self.path"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_saves_lose_no_updates() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Arc::new(dir.path().join(SETTINGS_STORE_FILE));
        atomic_write(&path, b"{}").expect("seed");

        let mut tasks = Vec::new();
        for i in 0..32 {
            let path = path.clone();
            tasks.push(tauri::async_runtime::spawn(async move {
                one_writer(|| {
                    let mut map: Map<String, Value> =
                        serde_json::from_slice(&fs::read(&*path).expect("read")).expect("json");
                    // Widen the read-modify-write window.
                    std::thread::sleep(Duration::from_millis(2));
                    map.insert(format!("key{i}"), Value::from(i));
                    atomic_write(&path, &serde_json::to_vec(&map).expect("ser")).expect("write");
                })
                .await;
            }));
        }
        for task in tasks {
            task.await.expect("task");
        }

        let map = read_json(&path);
        let map = map.as_object().expect("object");
        assert_eq!(map.len(), 32);
        assert!(leftover_temps(dir.path()).is_empty());
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Nested {
        depth: u32,
        label: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Section {
        enabled: bool,
        name: String,
        count: u32,
        nested: Nested,
    }

    impl Default for Section {
        fn default() -> Self {
            Self {
                enabled: false,
                name: "default".into(),
                count: 7,
                nested: Nested {
                    depth: 1,
                    label: "deep".into(),
                },
            }
        }
    }

    #[test]
    fn unknown_missing_and_wrong_fields_fall_back_one_by_one() {
        let stored = serde_json::json!({
            "enabled": true,            // good
            "count": "seven",           // wrong type
            "from_a_newer_juno": [1],   // unknown
            "nested": { "depth": "x", "label": "kept" }
            // "name" is missing
        });
        let section: Section = lenient("test", &stored);
        assert_eq!(
            section,
            Section {
                enabled: true,
                name: "default".into(),
                count: 7,
                nested: Nested {
                    depth: 1,
                    label: "kept".into(),
                },
            }
        );
    }

    #[test]
    fn a_healthy_section_reads_exactly_as_before() {
        let stored = serde_json::json!({
            "enabled": true, "name": "mine", "count": 3,
            "nested": { "depth": 9, "label": "x" }
        });
        let section: Section = lenient("test", &stored);
        assert_eq!(section.name, "mine");
        assert_eq!(section.nested.depth, 9);
    }

    #[test]
    fn a_real_section_keeps_its_good_fields() {
        // CLISettings has no serde defaults, so a single missing or
        // wrong-typed field used to reset all of it.
        let stored = serde_json::json!({
            "logging_enabled": true,
            "log_level": 5,
            "command_timeout": 99,
            "brand_new_field": {"a": 1}
        });
        let cli: crate::settings::CLISettings = lenient("cli", &stored);
        let defaults = crate::settings::CLISettings::default();
        assert!(cli.logging_enabled);
        assert_eq!(cli.command_timeout, 99);
        assert_eq!(cli.log_level, defaults.log_level);
        assert_eq!(cli.max_history_entries, defaults.max_history_entries);
    }

    #[test]
    fn a_truncated_settings_file_is_replaced_silently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(SETTINGS_STORE_FILE);
        let truncated = br#"{"keyboard_shortcuts": {"agent_mode": "Cmd+Sh"#;
        fs::write(&path, truncated).expect("seed");
        // A healthy neighbour is left alone.
        let bar = dir.path().join(store_files::BAR_POSITION);
        fs::write(&bar, br#"{"x": 1}"#).expect("seed bar");

        let replaced = guard_dir(dir.path());

        assert_eq!(replaced, vec![SETTINGS_STORE_FILE]);
        assert_eq!(read_json(&path), serde_json::json!({}));
        assert_eq!(read_json(&bar), serde_json::json!({"x": 1}));
        // Nothing is kept beside it: only the two store files remain.
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let mut expected = vec![
            SETTINGS_STORE_FILE.to_string(),
            store_files::BAR_POSITION.to_string(),
        ];
        expected.sort();
        assert_eq!(names, expected);
    }

    #[test]
    fn a_healthy_or_missing_file_is_left_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(SETTINGS_STORE_FILE);
        assert!(!recover_if_damaged(&path));
        fs::write(&path, br#"{"autostart_enabled": true}"#).expect("seed");
        assert!(!recover_if_damaged(&path));
        assert_eq!(
            read_json(&path),
            serde_json::json!({"autostart_enabled": true})
        );
    }
}
