/**
 * Curated Shiki highlighter shared by every code surface in Juno.
 *
 * Why not `import { createHighlighter } from "shiki"`: the full bundle
 * registers all 200+ grammars as lazy chunks, which ships ~10 MB of grammar
 * JS in dist/assets that a desktop AI assistant never renders. This module
 * uses `shiki/core` with an explicit grammar set and explicit themes, so only
 * those chunks exist in the build. Anything outside the set degrades to plain
 * text (one token per line, no colour) instead of throwing.
 *
 * The JavaScript regex engine is used instead of the Oniguruma WASM engine:
 * every curated grammar compiles under it in strict mode (verified 2026-09-14
 * against shiki 3.23), it needs no `.wasm` chunk, and it is the engine the
 * previous `@streamdown/code` plugin already used for chat code fences.
 */

import type { HighlighterCore, ThemedToken } from "shiki/core";
import { createHighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";

/**
 * Grammars Juno ships. Each entry is a dynamic import so Vite emits one lazy
 * chunk per grammar and nothing loads until a code block asks for it.
 *
 * Chosen for what an AI assistant that drives macOS shows in chat: shell
 * commands, the web stack, the languages Juno itself is written in (Rust,
 * TypeScript), config formats, SQL, Swift (macOS native code), Go, diffs,
 * and AppleScript (Juno automates apps through it).
 */
export const CURATED_LANGUAGES = {
  applescript: () => import("shiki/langs/applescript.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  jsonc: () => import("shiki/langs/jsonc.mjs"),
  jsx: () => import("shiki/langs/jsx.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  rust: () => import("shiki/langs/rust.mjs"),
  shellscript: () => import("shiki/langs/shellscript.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  typescript: () => import("shiki/langs/typescript.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
} as const;

export type CuratedLanguage = keyof typeof CURATED_LANGUAGES;

/** Shiki's built-in no-grammar language: one token per line, theme fg only. */
export const PLAIN_TEXT_LANGUAGE = "text";

export type ResolvedLanguage = CuratedLanguage | typeof PLAIN_TEXT_LANGUAGE;

/**
 * Fence-label aliases, mirroring shiki's `bundledLanguagesInfo` aliases for
 * the curated set (the full bundle resolved these for us before), plus the
 * spellings shiki treats as plain text.
 */
const LANGUAGE_ALIASES: Record<string, ResolvedLanguage> = {
  bash: "shellscript",
  cjs: "javascript",
  console: "shellscript",
  cts: "typescript",
  js: "javascript",
  md: "markdown",
  mjs: "javascript",
  mts: "typescript",
  plain: PLAIN_TEXT_LANGUAGE,
  plaintext: PLAIN_TEXT_LANGUAGE,
  py: "python",
  rs: "rust",
  sh: "shellscript",
  shell: "shellscript",
  ts: "typescript",
  txt: PLAIN_TEXT_LANGUAGE,
  yml: "yaml",
  zsh: "shellscript",
};

const isCuratedLanguage = (id: string): id is CuratedLanguage =>
  Object.prototype.hasOwnProperty.call(CURATED_LANGUAGES, id);

/**
 * Map any fence label to a grammar Juno ships, or to plain text.
 * Never throws: an unknown, empty, or oddly-cased label becomes `text`.
 */
export const resolveLanguage = (
  language: string | null | undefined
): ResolvedLanguage => {
  const normalized = (language ?? "").trim().toLowerCase();
  if (normalized === "" || normalized === PLAIN_TEXT_LANGUAGE) {
    return PLAIN_TEXT_LANGUAGE;
  }
  if (isCuratedLanguage(normalized)) {
    return normalized;
  }
  return LANGUAGE_ALIASES[normalized] ?? PLAIN_TEXT_LANGUAGE;
};

/** Every label `resolveLanguage` highlights (ids plus aliases). */
export const SUPPORTED_LANGUAGE_LABELS: readonly string[] = [
  ...Object.keys(CURATED_LANGUAGES),
  ...Object.keys(LANGUAGE_ALIASES).filter(
    (alias) => LANGUAGE_ALIASES[alias] !== PLAIN_TEXT_LANGUAGE
  ),
];

/** Themes used everywhere; dual-theme output drives the `--shiki-dark` vars. */
export const LIGHT_THEME = "github-light";
export const DARK_THEME = "github-dark";
export const CODE_THEMES: [typeof LIGHT_THEME, typeof DARK_THEME] = [
  LIGHT_THEME,
  DARK_THEME,
];

export interface TokenizedCode {
  tokens: ThemedToken[][];
  fg: string;
  bg: string;
}

// One highlighter for the whole app. Grammars are added on demand.
let highlighterPromise: Promise<HighlighterCore> | null = null;

const getHighlighter = (): Promise<HighlighterCore> => {
  highlighterPromise ??= createHighlighterCore({
    engine: createJavaScriptRegexEngine({ forgiving: true }),
    langs: [],
    themes: [
      import("shiki/themes/github-light.mjs"),
      import("shiki/themes/github-dark.mjs"),
    ],
  });
  return highlighterPromise;
};

// Per-grammar load promise so concurrent blocks share one import.
const languageLoads = new Map<CuratedLanguage, Promise<void>>();

const ensureLanguage = async (
  highlighter: HighlighterCore,
  language: ResolvedLanguage
): Promise<void> => {
  if (language === PLAIN_TEXT_LANGUAGE) {
    return;
  }
  if (highlighter.getLoadedLanguages().includes(language)) {
    return;
  }
  let load = languageLoads.get(language);
  if (!load) {
    load = highlighter.loadLanguage(CURATED_LANGUAGES[language]());
    languageLoads.set(language, load);
  }
  await load;
};

/**
 * Tokenize `code` with dual themes. Resolves aliases, loads the grammar on
 * first use, and falls back to plain text for anything outside the set or
 * any grammar that fails to load, so callers always get renderable tokens.
 */
export const tokenizeCode = async (
  code: string,
  language: string | null | undefined
): Promise<TokenizedCode> => {
  const highlighter = await getHighlighter();
  let lang = resolveLanguage(language);
  try {
    await ensureLanguage(highlighter, lang);
  } catch (error) {
    console.error(`Failed to load grammar "${lang}", rendering plain:`, error);
    lang = PLAIN_TEXT_LANGUAGE;
  }
  const result = highlighter.codeToTokens(code, {
    lang,
    themes: { dark: DARK_THEME, light: LIGHT_THEME },
  });
  return {
    bg: result.bg ?? "transparent",
    fg: result.fg ?? "inherit",
    // Grammars emit `[]` for a blank line but the plain-text path emits one
    // empty token. Renderers key "blank line" off an empty array, so make the
    // plain path match or blank lines collapse in unknown-language blocks.
    tokens: result.tokens.map((line) =>
      line.length === 1 && line[0].content === "" ? [] : line
    ),
  };
};

/** Plain tokens for immediate paint while the highlighter loads. */
export const createRawTokens = (code: string): TokenizedCode => ({
  bg: "transparent",
  fg: "inherit",
  tokens: code.split("\n").map((line) =>
    line === ""
      ? []
      : [
          {
            color: "inherit",
            content: line,
            offset: 0,
          },
        ]
  ),
});

// Result cache + subscribers so many blocks with the same code share work.
const tokensCache = new Map<string, TokenizedCode>();
const subscribers = new Map<string, Set<(result: TokenizedCode) => void>>();

const getTokensCacheKey = (code: string, language: ResolvedLanguage) => {
  const start = code.slice(0, 100);
  const end = code.length > 100 ? code.slice(-100) : "";
  return `${language}:${code.length}:${start}:${end}`;
};

/**
 * Synchronous cache lookup with an async fill. Returns the cached tokens when
 * present; otherwise returns `null`, starts highlighting, and calls
 * `callback` once with the result. Never rejects and never throws on an
 * unknown language.
 */
export const highlightCode = (
  code: string,
  language: string | null | undefined,
  // oxlint-disable-next-line eslint-plugin-promise(prefer-await-to-callbacks)
  callback?: (result: TokenizedCode) => void
): TokenizedCode | null => {
  const resolved = resolveLanguage(language);
  const cacheKey = getTokensCacheKey(code, resolved);

  const cached = tokensCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  if (callback) {
    let subs = subscribers.get(cacheKey);
    if (!subs) {
      subs = new Set();
      subscribers.set(cacheKey, subs);
    }
    subs.add(callback);
  }

  tokenizeCode(code, resolved)
    // oxlint-disable-next-line eslint-plugin-promise(prefer-await-to-then)
    .then((tokenized) => {
      tokensCache.set(cacheKey, tokenized);
      const subs = subscribers.get(cacheKey);
      if (subs) {
        for (const sub of subs) {
          sub(tokenized);
        }
        subscribers.delete(cacheKey);
      }
    })
    // oxlint-disable-next-line eslint-plugin-promise(prefer-await-to-then), eslint-plugin-promise(prefer-await-to-callbacks)
    .catch((error) => {
      // tokenizeCode already falls back to plain text; this only fires if the
      // highlighter itself (core or themes) failed to initialise. Leave the
      // raw tokens on screen rather than a blank block.
      console.error("Failed to highlight code:", error);
      subscribers.delete(cacheKey);
    });

  return null;
};
