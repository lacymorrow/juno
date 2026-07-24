//! # UI Constants
//!
//! User interface related constants including window labels and breakpoints.

pub mod window_labels {
    pub const MAIN: &str = "main";
    pub const FLOATING_BAR: &str = "floating-bar";
    pub const FLOATING_PANEL: &str = "floating-panel";
    pub const ONBOARDING: &str = "onboarding";
    pub const SETTINGS: &str = "settings";
    pub const APP_BAR: &str = "app-bar";
    pub const VOICE_BAR: &str = "voice-bar";
    pub const DYNAMIC_BAR: &str = "dynamic-bar";
}

/// Agent session identity colors (LAC-1432 / LAC-2830 spec section 2).
/// Fixed 8-color palette assigned slot-by-slot as parallel agent sessions spawn.
/// Identity colors are used for cursor overlay rings, roster dots, and labels —
/// never for status dots, which keep the existing blue/yellow/green semantics.
/// NOTE: keep doc comments in this module free of curly braces — the
/// generate-ts-constants.js module parser drops constants that follow one.
pub mod agent_session_colors {
    pub const SLOT_0: &str = "#3B82F6"; // blue
    pub const SLOT_1: &str = "#10B981"; // emerald
    pub const SLOT_2: &str = "#F59E0B"; // amber
    pub const SLOT_3: &str = "#F43F5E"; // rose
    pub const SLOT_4: &str = "#8B5CF6"; // violet
    pub const SLOT_5: &str = "#06B6D4"; // cyan
    pub const SLOT_6: &str = "#F97316"; // orange
    pub const SLOT_7: &str = "#EC4899"; // pink
}

/// UI element IDs used for element targeting and interactions
pub mod element_ids {
    pub const FLOATING_BAR: &str = "floating-bar";
    pub const APP_BAR: &str = "app-bar";
    pub const VOICE_AI_BAR: &str = "voice-ai-bar";
    pub const DYNAMIC_BAR: &str = "dynamic-bar";
    pub const FLOATING_PANEL: &str = "floating-panel";
}

/// Bar appearance/style identifiers
/// These values are persisted in settings and selected by the user in Settings → General.
pub mod bar_appearances {
    pub const FLOATING: &str = "floating";
    pub const APP: &str = "app";
    pub const VOICE_AI: &str = "voice_ai";
    pub const DYNAMIC: &str = "dynamic";
    pub const ORB: &str = "orb";
    pub const PERSONA: &str = "persona";
}

/// UI state constants for bar state management
/// These values are used by the backend BarState enum and must match frontend expectations
pub mod bar_states {
    pub const DEFAULT: &str = "default";
    pub const EXPANDING: &str = "expanding";
    pub const INPUT: &str = "input";
    pub const SHRINKING: &str = "shrinking";
    pub const SUBMITTING: &str = "submitting";
    pub const LOADING: &str = "loading";
    pub const SUCCESS: &str = "success";
    pub const ERROR: &str = "error";
    pub const SPEAKING: &str = "speaking";
    pub const LISTENING: &str = "listening";
    pub const TRANSCRIBING: &str = "transcribing";
    pub const DICTATING: &str = "dictating";
    pub const DICTATION_READY: &str = "dictation_ready";
    pub const ALWAYS_LISTENING: &str = "always_listening";
    pub const FINISHING: &str = "finishing";
    pub const AGENT_RESPONDING: &str = "agent_responding";
    pub const STOPPING: &str = "stopping";
}

/// Voice mode constants
/// These values are used by the frontend VoiceContext and must match frontend expectations
pub mod voice_modes {
    pub const IDLE: &str = "idle";
    pub const AGENT: &str = "agent";
    pub const DICTATION: &str = "dictation";
    pub const SPEAKING: &str = "speaking";
    pub const ALWAYS_LISTENING: &str = "always_listening";
}

/// Agent status constants
/// These values are used by the frontend agent state management and must match frontend expectations
pub mod agent_status {
    pub const IDLE: &str = "idle";
    pub const DICTATING: &str = "dictating";
    pub const LISTENING: &str = "listening";
    pub const THINKING: &str = "thinking";
    pub const RESPONDING: &str = "responding";
    pub const ERROR: &str = "error";
    pub const WORKING: &str = "working";
    pub const FINISHED: &str = "Finished";
    pub const FAILED: &str = "Failed";
    pub const CANCELLED: &str = "Cancelled";
    pub const OFFLINE: &str = "Offline";
    pub const PROCESSING: &str = "processing";
    pub const SPEAKING: &str = "speaking";
    pub const INPUT: &str = "input";
    pub const SUCCESS: &str = "success";
    pub const RESPONSE: &str = "response";
}

/// UI interaction types
/// These values are used by the frontend UI interaction handlers
pub mod interaction_types {
    pub const CLICK: &str = "click";
    pub const FOCUS: &str = "focus";
    pub const BLUR: &str = "blur";
    pub const HOVER: &str = "hover";
    pub const INPUT: &str = "input";
    pub const SUBMIT: &str = "submit";
    pub const ESCAPE: &str = "escape";
    pub const ENTER: &str = "enter";
    pub const INPUT_CHANGE: &str = "input_change";
    pub const INITIALIZE: &str = "initialize";
    pub const SET_CLICK_THROUGH: &str = "set_click_through";
    pub const SET_LEVEL: &str = "set_level";
}

// UI layout constants (moved from frontend constants.ts)
pub const MOBILE_BREAKPOINT: i32 = 768;
pub const PERCENTAGE_MULTIPLIER: f64 = 100.0;
pub const SCROLL_WHEEL_EVENT_LINE_SCROLL: i32 = 120;
pub const DOUBLE_CLICK_INTERVAL_MS: u64 = 50;
pub const MAX_TREE_SEARCH_DEPTH: usize = 100;

/// UI text display constants
pub mod text_display {
    /// Maximum characters to show in key press visualization text
    pub const MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH: usize = 30;

    /// Maximum characters to show in UI preview text
    pub const MAX_UI_PREVIEW_TEXT_LENGTH: usize = 50;
}

/// Standard resolution constants for Anthropic Computer Use API compliance.
/// Screenshots must be scaled to these standard resolutions per specification.
///
/// Opus 4.5+ models support up to 2,576px on the long edge with 1:1 pixel
/// coordinates (no scale-factor conversion needed), so we offer a high-res
/// tier alongside the legacy low-res resolutions for older models.
pub mod standard_resolutions {
    /// Extended Graphics Array - 1024x768
    pub const XGA: (u32, u32) = (1024, 768);

    /// Wide Extended Graphics Array - 1280x800
    pub const WXGA: (u32, u32) = (1280, 800);

    /// Full Wide Extended Graphics Array - 1366x768
    pub const FWXGA: (u32, u32) = (1366, 768);

    // --- High-resolution tier (Opus 4.5+ / computer_20251124) ---
    // Anthropic docs: "Opus 4.7 supports up to 2576 pixels on the long edge,
    // and its coordinates are 1:1 with image pixels (no scale-factor
    // conversion required)."  These apply to all models using the
    // computer_20251124 tool type.

    /// High-res 16:10 — matches MacBook Pro 14" logical resolution
    pub const HD_WXGA: (u32, u32) = (1680, 1050);

    /// High-res 16:9 — standard 1080p
    pub const HD_1080: (u32, u32) = (1920, 1080);

    /// Max Anthropic resolution — 2,576px long edge, 16:10 aspect
    pub const ULTRA_HD: (u32, u32) = (2576, 1610);

    /// Ultra-wide 21:9 — 2560x1080, just under the 2576px long-edge cap.
    /// Aspect ratio 2.370 matches 21:9 monitors (2.333) far better than
    /// any 16:9 or 16:10 candidate (which have diff ≈ 0.55).
    pub const UW_1080: (u32, u32) = (2560, 1080);

    /// Legacy resolutions for older models (pre-Opus 4.5)
    pub const LEGACY_RESOLUTIONS: [(u32, u32); 3] = [XGA, WXGA, FWXGA];

    /// High-resolution options for Opus 4.5+ models
    pub const HIGH_RES_RESOLUTIONS: [(u32, u32); 4] = [HD_WXGA, HD_1080, ULTRA_HD, UW_1080];

    /// All supported standard resolutions (legacy + high-res)
    pub const ALL_RESOLUTIONS: [(u32, u32); 7] =
        [XGA, WXGA, FWXGA, HD_WXGA, HD_1080, ULTRA_HD, UW_1080];

    /// Whether a model supports high-resolution screenshots (2,576px).
    /// Returns true for Opus 4.5+ models that use computer_20251124.
    /// Also matches Claude CLI aliases ("opus", "sonnet") which resolve
    /// to current-gen models that support high-res.
    pub fn supports_high_res(model: &str) -> bool {
        use crate::agent::providers::types::model_ids;
        model_ids::OPUS_4_5_PLUS_MODELS.contains(&model) || matches!(model, "opus" | "sonnet")
    }

    /// Select the best standard resolution for a given display and model.
    ///
    /// For high-res capable models (Opus 4.5+), picks the largest resolution
    /// that fits within 2,576px on the long edge while matching aspect ratio.
    /// For legacy models, picks from the original XGA/WXGA/FWXGA set.
    pub fn select_best_resolution(display_width: u32, display_height: u32) -> (u32, u32) {
        // Default to legacy — callers that are model-aware should use
        // select_best_resolution_for_model() instead.
        select_best_resolution_from_set(display_width, display_height, &LEGACY_RESOLUTIONS)
    }

    /// Model-aware resolution selection.
    pub fn select_best_resolution_for_model(
        display_width: u32,
        display_height: u32,
        model: &str,
    ) -> (u32, u32) {
        let candidates = if supports_high_res(model) {
            &ALL_RESOLUTIONS[..]
        } else {
            &LEGACY_RESOLUTIONS[..]
        };
        select_best_resolution_from_set(display_width, display_height, candidates)
    }

    /// Pick the candidate resolution whose aspect ratio is closest to the display.
    /// Only considers resolutions that fit within the display dimensions.
    /// Among ties, prefer the larger resolution (better click accuracy).
    fn select_best_resolution_from_set(
        display_width: u32,
        display_height: u32,
        candidates: &[(u32, u32)],
    ) -> (u32, u32) {
        if display_width == 0 || display_height == 0 || candidates.is_empty() {
            return XGA; // Default fallback
        }

        let display_aspect = display_width as f64 / display_height as f64;

        // Only consider resolutions that fit within the display
        let fitting: Vec<(u32, u32)> = candidates
            .iter()
            .copied()
            .filter(|(w, h)| *w <= display_width && *h <= display_height)
            .collect();

        // Fall back to full candidate list if none fit (very small display)
        let pool = if fitting.is_empty() {
            candidates
        } else {
            &fitting
        };

        let selected = pool
            .iter()
            .copied()
            .min_by(|a, b| {
                let diff_a = (display_aspect - a.0 as f64 / a.1 as f64).abs();
                let diff_b = (display_aspect - b.0 as f64 / b.1 as f64).abs();
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    // Tie-break: prefer larger resolution
                    .then_with(|| (b.0 * b.1).cmp(&(a.0 * a.1)))
            })
            .unwrap_or(XGA);

        let selected_aspect = selected.0 as f64 / selected.1 as f64;
        let aspect_diff = (display_aspect - selected_aspect).abs();
        tracing::debug!(
            "Resolution selection: display {}x{} (aspect {:.3}) → {}x{} (aspect {:.3}, diff {:.3})",
            display_width,
            display_height,
            display_aspect,
            selected.0,
            selected.1,
            selected_aspect,
            aspect_diff
        );
        if aspect_diff > 0.2 {
            tracing::warn!(
                "Aspect ratio mismatch: display {:.3} vs standard {:.3} (diff {:.3}) — \
                 coordinates may be slightly imprecise for display {}x{}",
                display_aspect,
                selected_aspect,
                aspect_diff,
                display_width,
                display_height
            );
        }

        selected
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_4_3_display_selects_xga() {
            // 1024×768 is 4:3 — XGA should win
            let res = select_best_resolution(1024, 768);
            assert_eq!(res, XGA, "4:3 display should select XGA");
        }

        #[test]
        fn test_16_10_display_selects_wxga() {
            // 1440×900 is 16:10 — WXGA should win over XGA and FWXGA
            let res = select_best_resolution(1440, 900);
            assert_eq!(res, WXGA, "16:10 display should select WXGA");
        }

        #[test]
        fn test_macbook_pro_16_10_selects_wxga() {
            // MacBook Pro 16" logical resolution (1728×1080) is close to 16:10
            let res = select_best_resolution(1728, 1080);
            assert_eq!(res, WXGA, "MacBook Pro 16\" (1728×1080) should select WXGA");
        }

        #[test]
        fn test_16_9_display_selects_fwxga() {
            // 1920×1080 is 16:9 — FWXGA should win
            let res = select_best_resolution(1920, 1080);
            assert_eq!(res, FWXGA, "16:9 display should select FWXGA");
        }

        #[test]
        fn test_high_res_16_10_selects_hd_wxga_for_opus() {
            // Opus 4.5+ on 16:10 should get HD_WXGA (1680×1050) over legacy WXGA
            let res = select_best_resolution_for_model(1728, 1080, "opus");
            assert_eq!(
                res, HD_WXGA,
                "Opus on 16:10 should select HD_WXGA (1680×1050)"
            );
        }

        #[test]
        fn test_high_res_16_9_selects_hd_1080_for_opus() {
            // Opus 4.5+ on a 16:9 4K display should get HD_1080 (1920×1080)
            let res = select_best_resolution_for_model(3840, 2160, "opus");
            assert_eq!(
                res, HD_1080,
                "Opus on 16:9 4K should select HD_1080 (1920×1080)"
            );
        }

        #[test]
        fn test_ultra_wide_21_9_selects_uw_1080_for_opus() {
            // Ultra-wide 3440×1440 (21:9) should pick UW_1080 (2560×1080)
            let res = select_best_resolution_for_model(3440, 1440, "opus");
            assert_eq!(
                res, UW_1080,
                "Ultra-wide 21:9 (3440×1440) should select UW_1080 (2560×1080)"
            );
        }

        #[test]
        fn test_ultra_wide_2560x1080_selects_uw_1080_for_opus() {
            // 2560×1080 native ultra-wide
            let res = select_best_resolution_for_model(2560, 1080, "opus");
            assert_eq!(
                res, UW_1080,
                "Native 2560×1080 ultra-wide should select UW_1080"
            );
        }

        #[test]
        fn test_zero_dimensions_returns_xga_fallback() {
            let res = select_best_resolution(0, 0);
            assert_eq!(res, XGA, "Zero dimensions should fall back to XGA");
        }

        #[test]
        fn test_very_small_display_falls_back_to_candidates() {
            // Smaller than any standard resolution — should still return something
            let res = select_best_resolution(640, 480);
            assert_eq!(
                res, XGA,
                "640×480 (4:3) should fall back to XGA (closest aspect)"
            );
        }

        #[test]
        fn test_legacy_model_ignores_high_res_candidates() {
            // A legacy model on a large 16:10 display should still get legacy WXGA
            let res = select_best_resolution_for_model(2560, 1600, "claude-2");
            assert_eq!(
                res, WXGA,
                "Legacy model should pick from LEGACY_RESOLUTIONS only"
            );
        }

        #[test]
        fn test_uw_1080_aspect_beats_hd_1080_for_ultra_wide() {
            // Verify UW_1080 aspect (2.370) is closer to 21:9 (2.333) than HD_1080 (1.778)
            let uw_diff = (2560.0 / 1080.0 - 3440.0 / 1440.0_f64).abs();
            let hd_diff = (1920.0 / 1080.0 - 3440.0 / 1440.0_f64).abs();
            assert!(uw_diff < hd_diff,
                "UW_1080 should have a smaller aspect diff ({:.3}) than HD_1080 ({:.3}) for 3440×1440",
                uw_diff, hd_diff);
        }
    }
}
