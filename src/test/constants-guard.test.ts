/**
 * Guards for the single-source-of-truth rule: every command name, event name
 * and shared identifier is defined once, in src-tauri/src/constants, and the
 * frontend reads it from the generated constants file. A string typed out by
 * hand is a copy, and copies go stale.
 */
import fs from "fs";
import path from "path";
import { describe, expect, it } from "vitest";
import {
  API_ENDPOINTS,
  APP_IDENTITY,
  COMMANDS,
  PORTS,
} from "@/lib/constants.generated";
// @ts-expect-error: plain JS build script, no type declarations
import { generateTypeScript, parseRustConstants } from "../../scripts/generate-ts-constants.js";

const ROOT = path.resolve(__dirname, "../..");
const read = (rel: string) => fs.readFileSync(path.join(ROOT, rel), "utf8");

function sourceFiles(dir: string): string[] {
  return fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true }).flatMap((entry) => {
    const rel = path.join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === "__tests__" ? [] : sourceFiles(rel);
    if (!/\.tsx?$/.test(entry.name)) return [];
    if (/\.(test|spec)\.tsx?$/.test(entry.name) || rel.endsWith("constants.generated.ts")) return [];
    return [rel];
  });
}

/**
 * Raw names that are allowed, each with its reason. Keep this short.
 * `src/lib/ui-api.ts` and its only importer `src/components/AppBar.tsx` are
 * not rendered anywhere, and these commands are not registered in lib.rs, so
 * calling them would fail. They stay listed until that module is deleted.
 */
const ALLOWED_RAW: Record<string, string[]> = {
  "src/lib/ui-api.ts": [
    "ui_get_state",
    "ui_set_state",
    "ui_get_config",
    "ui_set_config",
    "ui_resize_window",
    "ui_move_window",
    "ui_show_window",
    "ui_hide_window",
    "ui_set_click_through",
    "ui_set_window_level",
  ],
};

describe("constants guard", () => {
  it("calls invoke, listen and emit with constants, never a typed-out name", () => {
    // The first argument is a string literal: "x", 'x' or `x`.
    const rawCall =
      /\b(invoke|invokeCommand|safeInvoke|listen|once|emit|useEventListener)(<[^()]*?>)?\(\s*(['"`])([^'"`]*)\3/g;
    const offenders: string[] = [];
    for (const file of sourceFiles("src")) {
      const text = read(file);
      for (const match of text.matchAll(rawCall)) {
        const name = match[4];
        if (ALLOWED_RAW[file]?.includes(name)) continue;
        const line = text.slice(0, match.index).split("\n").length;
        offenders.push(`${file}:${line} ${match[1]}("${name}")`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it("names only commands that lib.rs registers", () => {
    const lib = read("src-tauri/src/lib.rs");
    const start = lib.indexOf("generate_handler![");
    const end = lib.indexOf("])", start);
    const registered = new Set(
      lib
        .slice(start, end)
        .replace(/\/\/.*$/gm, "")
        .split(/[\s,[\]]+/)
        .map((item) => item.split("::").pop() ?? ""),
    );
    const unregistered = Object.entries(COMMANDS)
      .filter(([, command]) => !registered.has(command))
      .map(([key, command]) => `COMMANDS.${key} = ${command}`);
    expect(unregistered).toEqual([]);
  });

  it("keeps constants.generated.ts in step with the Rust constants", () => {
    // `bun run generate-constants` rewrites it; this fails when someone
    // changed a Rust constant and did not regenerate.
    const fresh = generateTypeScript(parseRustConstants());
    expect(read("src/lib/constants.generated.ts")).toBe(fresh);
  });

  it("agrees with tauri.conf.json on the values both define", () => {
    const conf = JSON.parse(read("src-tauri/tauri.conf.json"));
    expect(conf.identifier).toBe(APP_IDENTITY.BUNDLE_IDENTIFIER);
    expect(new URL(conf.build.devUrl).port).toBe(String(PORTS.VITE_DEV_PORT));
    expect(conf.plugins.updater.endpoints).toEqual([API_ENDPOINTS.ENDPOINTS_UPDATE_FEED_STABLE]);
  });
});
