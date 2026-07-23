import type { BarAppearance } from "@/components/bar/barAppearance";

/**
 * Configuration for the floating bar UI element.
 * This type should match the Rust FloatingBarConfig struct.
 */
export interface FloatingBarConfig {
  show_voice_indicator: boolean;
  enable_animations: boolean;
  auto_hide: boolean;
  auto_hide_delay: number;
  opacity: number;
  bar_appearance: BarAppearance;
}

/**
 * Notch (or fallback pill) geometry for the notch bar appearance.
 * This type should match the Rust NotchGeometry struct
 * (src-tauri/src/platform/macos.rs, notch_layout module).
 * All values are logical points.
 */
export interface NotchGeometry {
  has_notch: boolean;
  notch_width: number;
  notch_height: number;
  menu_bar_height: number;
  canvas_width: number;
  canvas_height: number;
}