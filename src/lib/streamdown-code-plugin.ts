/**
 * Streamdown `code` plugin backed by Juno's curated highlighter.
 *
 * Drop-in for `@streamdown/code`, which imports `bundledLanguages` from the
 * full `shiki` bundle and therefore ships every grammar as a lazy chunk. This
 * one shares the single highlighter and token cache in `code-highlighter.ts`,
 * so chat fences and tool-call code blocks load each grammar once.
 *
 * Themes are fixed to Juno's github-light/github-dark pair; the `themes`
 * option Streamdown passes is the pair this plugin returns from `getThemes`,
 * so it is not consulted.
 */

import type { BundledLanguage } from "shiki";
import type { CodeHighlighterPlugin } from "streamdown";

import {
  CODE_THEMES,
  highlightCode,
  PLAIN_TEXT_LANGUAGE,
  resolveLanguage,
  SUPPORTED_LANGUAGE_LABELS,
} from "./code-highlighter";

export const code: CodeHighlighterPlugin = {
  getSupportedLanguages: () => SUPPORTED_LANGUAGE_LABELS as BundledLanguage[],
  getThemes: () => CODE_THEMES,
  highlight: ({ code: source, language }, callback) =>
    highlightCode(source, language, callback),
  name: "shiki",
  supportsLanguage: (language) =>
    resolveLanguage(language) !== PLAIN_TEXT_LANGUAGE,
  type: "code-highlighter",
};
