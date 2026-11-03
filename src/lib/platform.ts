/**
 * Which desktop OS this page runs in, for the few surfaces that have to look
 * different there. Display only: behaviour is Rust's to decide.
 */
export const IS_WINDOWS =
  typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/**
 * Whether there is a voice to talk to. Mirrors Rust's
 * `platform::VOICE_AVAILABLE`: macOS only until the Windows voice step, and
 * where it is off no microphone is offered.
 */
export const VOICE_AVAILABLE = !IS_WINDOWS;

/**
 * The dark face of the bar's pill and chat pane. On macOS it is slightly
 * translucent over a blur; WebView2 cannot blur what is behind its window, so
 * on Windows a translucent face only lets the window underneath show through,
 * and the face is opaque instead.
 */
export const BAR_SURFACE_BG = IS_WINDOWS ? "bg-neutral-950" : "bg-neutral-950/90";
