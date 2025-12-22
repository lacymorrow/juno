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