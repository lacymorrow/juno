/**
 * Which desktop OS this page runs in, for the few surfaces that have to look
 * different there. Display only: behaviour is Rust's to decide.
 */
export const IS_WINDOWS =
  typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/**
 * The dark face of the bar's pill and chat pane. On macOS it is slightly
 * translucent over a blur; WebView2 cannot blur what is behind its window, so
 * on Windows a translucent face only lets the window underneath show through,
 * and the face is opaque instead.
 */
export const BAR_SURFACE_BG = IS_WINDOWS ? "bg-neutral-950" : "bg-neutral-950/90";
