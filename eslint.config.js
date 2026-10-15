/**
 * Juno's frontend lint gate. `bun run lint` locally; CI runs it as the first
 * step of the Frontend job, before vitest, because it is the cheapest gate.
 *
 * It lints exactly two things, because everything else is already covered:
 *
 *   1. React hooks correctness (eslint-plugin-react-hooks). This is the one
 *      class of bug neither `tsc` nor vitest can see: a hook called
 *      conditionally, or state set during render, typechecks, passes its
 *      tests, and then tears under React 19's concurrent rendering.
 *
 *   2. Juno's architectural boundary (CLAUDE.md: "Backend Owns Logic, Frontend
 *      is Display-Only"). The frontend may not record audio, play audio,
 *      persist settings, or open sockets. That rule has lived in prose across
 *      three CLAUDE.md files and been enforced by code review, which has not
 *      worked — every `eslint-disable-next-line` in the tree referencing this
 *      file is a place it was broken. These rules make it a gate, so the debt
 *      stops growing while the existing violations are paid off.
 *
 * ## This config is a ratchet, and that is deliberate
 *
 * eslint-plugin-react-hooks v7 ships the React Compiler's full rule set, and
 * `configs.recommended` reports 226 findings on Juno today. Turning that on
 * wholesale gives you a red CI check that nobody can make green, which is the
 * same as having no gate at all — worse, because people learn to ignore it.
 *
 * So the enforced set below is the subset that reports ZERO findings today. It
 * costs nothing, it can never be "temporarily" lowered to land a branch, and it
 * still contains the two rules that catch the worst React bugs there are
 * (`rules-of-hooks`, `set-state-in-render`). The deferred rules are listed by
 * name with their finding count and their issue, so the map is visible rather
 * than implied by what is missing. Turn each one on in its own PR as the
 * findings get fixed; never turn one on and suppress the findings.
 *
 * Rules are enumerated rather than spread from `configs.recommended` on
 * purpose: a plugin upgrade that adds a rule should not turn CI red on a branch
 * that changed no frontend code.
 *
 * Deliberately NOT here:
 *   - A formatter. The repo has never had one, and adding one means reformatting
 *     377 files in the same PR that adds the gate: an unreviewable diff.
 *   - typescript-eslint's rule sets. `bun run build` is `tsc && vite build` with
 *     `strict`, `noUnusedLocals` and `noUnusedParameters` on, so type-aware lint
 *     would re-report what CI already fails on, at several times the runtime.
 *   - eslint-plugin-react. Its rules are mostly JSX style, and the ones that
 *     were not died with React 19's automatic JSX transform.
 *   - `no-undef`. `tsc` owns undefined identifiers and already knows the DOM
 *     lib; ESLint would need a `globals` dependency to be told the same thing.
 *
 * ## Why @babel/eslint-parser and not @typescript-eslint/parser
 *
 * Juno is on TypeScript 7, and no published typescript-eslint parses it — it
 * refuses at import with "typescript-eslint does not support TS 7.0"
 * (github.com/typescript-eslint/typescript-eslint/issues/10940). Rather than
 * pin a second, side-by-side TypeScript 6 just to parse, this uses Babel's
 * parser, which is the better fit anyway:
 *
 *   - The React Compiler that powers these hooks rules IS a Babel plugin.
 *     Babel's AST is its native input.
 *   - It has no dependency on the `typescript` package, so it cannot be broken
 *     by a TypeScript major — which is the exact failure we are routing around.
 *   - The only thing typescript-eslint's parser adds over it is type
 *     information for type-aware rules, and those are deliberately out (above).
 *
 * Syntax plugins go in `parserOpts.plugins`, NOT in `presets`. Babel 8's
 * eslint-parser only parses, so preset-derived syntax never reaches
 * @babel/parser and every TypeScript file fails on `import type`.
 */

import babelParser from "@babel/eslint-parser";
import reactHooks from "eslint-plugin-react-hooks";

/** Shared Babel options; `plugins` differs between .ts and .tsx (see below). */
const babelOptionsFor = (plugins) => ({
  requireConfigFile: false,
  babelOptions: { babelrc: false, configFile: false, parserOpts: { plugins } },
});

/**
 * The enforced hooks rules: every rule in `configs.recommended` that reports
 * zero findings on Juno today. Promoted to `error` where recommended says
 * `warn`, because a gate that can emit a warning accumulates warnings.
 */
const HOOKS_ENFORCED = {
  // A hook called conditionally, in a loop, or outside a component. The
  // original rule, and still the one that causes the most damage.
  "react-hooks/rules-of-hooks": "error",
  // setState during render: an infinite render loop, or a torn first paint.
  "react-hooks/set-state-in-render": "error",
  // Errors thrown where no boundary can catch them.
  "react-hooks/error-boundaries": "error",
  // Module-level mutable state read during render.
  "react-hooks/globals": "error",
  // A library used in a way the compiler cannot make safe.
  "react-hooks/incompatible-library": "error",
  // Malformed "use memo" / "use no memo" directives, and the compiler's own
  // gating and config directives. Cheap, and they catch typos that would
  // otherwise silently disable the compiler for a whole file.
  "react-hooks/config": "error",
  "react-hooks/gating": "error",
  "react-hooks/unsupported-syntax": "error",
};

/**
 * Deferred, with today's finding count. Each is a real correctness rule and
 * each needs a sweep of its own. Fix the findings, then turn the rule on in
 * that same PR — see LAC-4500.
 *
 *   react-hooks/set-state-in-effect          72
 *   react-hooks/refs                         64
 *   react-hooks/exhaustive-deps              33
 *   react-hooks/immutability                 16
 *   react-hooks/preserve-manual-memoization  13
 *   react-hooks/purity                        7
 *   react-hooks/static-components              4
 *   react-hooks/use-memo                       1
 */
const HOOKS_DEFERRED = {
  "react-hooks/set-state-in-effect": "off",
  "react-hooks/refs": "off",
  "react-hooks/exhaustive-deps": "off",
  "react-hooks/immutability": "off",
  "react-hooks/preserve-manual-memoization": "off",
  "react-hooks/purity": "off",
  "react-hooks/static-components": "off",
  "react-hooks/use-memo": "off",
};

/**
 * Browser APIs the frontend must not touch, and the Rust that owns each. Every
 * message names the owner, because "banned" without "use this instead" only
 * teaches people to reach for a disable comment.
 */
const BANNED_GLOBALS = [
  {
    name: "localStorage",
    message:
      "Settings persistence is Rust's (Tauri Store, tauri_plugin_store::StoreExt). The frontend reads cached settings through invoke(), never localStorage — the backend has to run headlessly.",
  },
  {
    name: "sessionStorage",
    message:
      "Settings persistence is Rust's (Tauri Store, tauri_plugin_store::StoreExt). The frontend reads cached settings through invoke(), never sessionStorage.",
  },
  {
    name: "WebSocket",
    message:
      "WebSockets are Rust's (cloud/connector.rs, tokio-tungstenite). The frontend listens for Tauri events; it never opens a socket.",
  },
  {
    name: "AudioContext",
    message:
      "Audio playback is Rust's (tts/, the `say` command, ElevenLabs). The Web Audio API is banned in the frontend.",
  },
  {
    name: "webkitAudioContext",
    message:
      "Audio playback is Rust's (tts/, the `say` command, ElevenLabs). The Web Audio API is banned in the frontend.",
  },
];

/** The boundary rules. Identical for .ts and .tsx. */
const BOUNDARY_RULES = {
  "no-restricted-globals": ["error", ...BANNED_GLOBALS],

  "no-restricted-imports": [
    "error",
    {
      paths: [
        {
          name: "@tauri-apps/plugin-websocket",
          message:
            "Importing this breaks the build. WebSockets are Rust's (cloud/connector.rs); the frontend listens for Tauri events.",
        },
      ],
    },
  ],

  "no-restricted-syntax": [
    "error",
    {
      // `navigator.mediaDevices` in any form. Microphone capture lives in
      // tauri-plugin-voice-transcription so the native permission prompt, the
      // device selection and the Whisper pipeline are all one thing.
      selector:
        "MemberExpression[object.name='navigator'][property.name='mediaDevices']",
      message:
        "Microphone capture is Rust's (tauri-plugin-voice-transcription). navigator.mediaDevices / getUserMedia are banned in the frontend.",
    },
    {
      // Banned outright: unreliable on macOS with `transparent: true` +
      // `decorations: false`, which is every Juno window. Dragging goes
      // through the useDragWindow hook.
      selector: "JSXAttribute[name.name='data-tauri-drag-region']",
      message:
        "Banned on macOS transparent/undecorated windows — it silently drops the drag. Use the useDragWindow hook (startDragging()) instead.",
    },
  ],
};

/** One rule block, shared by the .ts and .tsx configs below. */
const RULES = { ...HOOKS_ENFORCED, ...HOOKS_DEFERRED, ...BOUNDARY_RULES };

export default [
  {
    ignores: [
      "dist/**",
      "target/**",
      "src-tauri/**",
      "backend-server/**",
      "marketing/**",
      "packages/**",
      "crates/**",
      "scripts/**",
      // Written by `bun run generate-constants` from the Rust constants. A
      // finding here is a codegen bug, and the fix belongs in the generator.
      "src/lib/constants.generated.ts",
    ],
  },

  {
    // Off, and coupled to the deferred rules above rather than to taste.
    //
    // The tree already carries 18 `eslint-disable-next-line
    // react-hooks/exhaustive-deps` comments, written against a lint setup that
    // never reached CI. With `exhaustive-deps` deferred, ESLint correctly reads
    // all 18 as suppressing nothing — so turning this on means either 18 errors
    // or deleting 18 comments that will be needed again the moment the rule is
    // enabled. Both are worse than leaving it off.
    //
    // Flip it to "error" in the same PR that enables `exhaustive-deps`
    // (LAC-4500), where the directives become meaningful again. Until then the
    // safeguard on the suppressions added below is that each one names its
    // issue, so the sweep that fixes the violation finds the comment.
    linterOptions: { reportUnusedDisableDirectives: "off" },
  },

  // .ts — no `jsx` syntax plugin, so angle-bracket type assertions (`<T>x`)
  // still parse. Enabling jsx here would read them as an unclosed element.
  {
    files: ["src/**/*.ts", "types/**/*.ts", "vite.config.ts"],
    languageOptions: {
      parser: babelParser,
      ecmaVersion: 2022,
      sourceType: "module",
      parserOptions: babelOptionsFor([["typescript", {}]]),
    },
    plugins: { "react-hooks": reactHooks },
    rules: RULES,
  },

  // .tsx — typescript + jsx. TSX has no angle-bracket assertion syntax, so
  // there is nothing for the jsx plugin to collide with.
  {
    files: ["src/**/*.tsx"],
    languageOptions: {
      parser: babelParser,
      ecmaVersion: 2022,
      sourceType: "module",
      parserOptions: babelOptionsFor([["typescript", {}], "jsx"]),
    },
    plugins: { "react-hooks": reactHooks },
    rules: RULES,
  },
];
