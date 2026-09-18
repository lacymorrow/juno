//! Which build this is.
//!
//! Every DMG used to be "Juno Demo 0.7.0", so two builds made on the same day
//! were indistinguishable once they left the machine, and a bug report could
//! only ever name a date. The facts below are compiled in by `build.rs` from
//! git at build time, `scripts/tauri-build.sh` puts the same facts in the
//! filename and in a manifest beside the DMG, and Settings shows the label so
//! it can be copied into a report.
//!
//! The version is the crate version. The build number is the commit count,
//! which is monotonic along a line of development; the short sha rides with it
//! because two branches can reach the same count.

use serde::Serialize;

/// What this build is. Every field is a compile-time constant.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    pub build: &'static str,
    pub commit: &'static str,
    pub branch: &'static str,
    pub built_at: &'static str,
    /// The tree had uncommitted changes, so `commit` does not fully describe
    /// what is running.
    pub dirty: bool,
    pub demo: bool,
    pub cohort: Option<&'static str>,
}

impl BuildInfo {
    /// One line, for Settings and for pasting into a bug report.
    ///
    /// `0.7.0 (2174) a19b4631 · demo sept-investors`
    pub fn label(&self) -> String {
        let mut out = format!("{} ({}) {}", self.version, self.build, self.commit);
        if self.dirty {
            out.push_str(" dirty");
        }
        if self.demo {
            out.push_str(" · demo");
            if let Some(cohort) = self.cohort {
                out.push(' ');
                out.push_str(cohort);
            }
        }
        out
    }
}

pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        build: env!("JUNO_BUILD_NUMBER"),
        commit: env!("JUNO_BUILD_COMMIT"),
        branch: env!("JUNO_BUILD_BRANCH"),
        built_at: env!("JUNO_BUILT_AT"),
        dirty: matches!(env!("JUNO_BUILD_DIRTY"), "true"),
        demo: crate::demo::is_demo_build(),
        cohort: crate::demo::cohort(),
    }
}

/// What Settings shows about this build. Cheap; reads compile-time strings.
#[tauri::command]
pub fn get_build_info() -> BuildInfo {
    build_info()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BuildInfo {
        BuildInfo {
            version: "0.7.0",
            build: "2174",
            commit: "a19b4631",
            branch: "main",
            built_at: "2026-09-17T14:00:00Z",
            dirty: false,
            demo: false,
            cohort: None,
        }
    }

    #[test]
    fn label_names_the_commit() {
        // The whole point: a report says which commit, not which day.
        assert_eq!(sample().label(), "0.7.0 (2174) a19b4631");
    }

    #[test]
    fn label_admits_an_uncommitted_tree() {
        let info = BuildInfo {
            dirty: true,
            ..sample()
        };
        assert!(info.label().contains("dirty"));
    }

    #[test]
    fn label_carries_the_demo_cohort() {
        // A demo build in the wild has to be traceable to the key it carries.
        let info = BuildInfo {
            demo: true,
            cohort: Some("sept-investors"),
            ..sample()
        };
        assert_eq!(info.label(), "0.7.0 (2174) a19b4631 · demo sept-investors");
    }

    #[test]
    fn label_handles_a_demo_without_a_cohort() {
        let info = BuildInfo {
            demo: true,
            ..sample()
        };
        assert_eq!(info.label(), "0.7.0 (2174) a19b4631 · demo");
    }

    #[test]
    fn this_build_describes_itself() {
        // Guards the build.rs wiring: if the env vars stop being emitted this
        // fails to compile, and if git fails at build time the placeholders
        // still produce a usable label.
        let info = build_info();
        assert!(!info.version.is_empty());
        assert!(!info.build.is_empty());
        assert!(!info.commit.is_empty());
        assert!(info.label().contains(info.commit));
    }
}
