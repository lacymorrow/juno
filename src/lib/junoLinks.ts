import { invoke } from "@tauri-apps/api/core";
import { defaultUrlTransform } from "streamdown";

import { COMMANDS } from "@/lib/constants.generated";

/**
 * `juno://` links in a reply go to Rust, which parses them and navigates
 * (src-tauri/src/deep_link.rs). The page decides nothing: it recognises the
 * scheme so the link is neither stripped nor sent to the browser.
 */
export function isJunoLink(url: string): boolean {
  return /^juno:\/\//i.test(url.trim());
}

/** Markdown keeps `juno://` hrefs; every other URL gets the library's usual check. */
export const junoUrlTransform: typeof defaultUrlTransform = (url, key, node) =>
  isJunoLink(url) ? url : defaultUrlTransform(url, key, node);

/**
 * Streamdown's link check. A Juno link is handed to Rust and answers "never
 * resolve": the library neither opens it nor asks "open this external link?".
 * Anything else gets the library's own confirmation as before.
 */
export function checkLink(url: string): Promise<boolean> | boolean {
  if (!isJunoLink(url)) return false;
  invoke(COMMANDS.SETTINGS_OPEN_JUNO_LINK, { url }).catch(() => {
    // A link that cannot be followed does nothing.
  });
  return new Promise<boolean>(() => {});
}

export const junoLinkSafety = { enabled: true, onLinkCheck: checkLink };
