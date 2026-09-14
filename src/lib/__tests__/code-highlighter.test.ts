import { describe, expect, it } from "vitest";

import {
  CURATED_LANGUAGES,
  highlightCode,
  PLAIN_TEXT_LANGUAGE,
  resolveLanguage,
  SUPPORTED_LANGUAGE_LABELS,
  tokenizeCode,
} from "../code-highlighter";
import { code as streamdownCodePlugin } from "../streamdown-code-plugin";

describe("resolveLanguage", () => {
  it("keeps curated ids as-is", () => {
    for (const id of Object.keys(CURATED_LANGUAGES)) {
      expect(resolveLanguage(id)).toBe(id);
    }
  });

  it("resolves the fence aliases the full shiki bundle used to resolve", () => {
    expect(resolveLanguage("sh")).toBe("shellscript");
    expect(resolveLanguage("bash")).toBe("shellscript");
    expect(resolveLanguage("zsh")).toBe("shellscript");
    expect(resolveLanguage("shell")).toBe("shellscript");
    expect(resolveLanguage("js")).toBe("javascript");
    expect(resolveLanguage("mjs")).toBe("javascript");
    expect(resolveLanguage("ts")).toBe("typescript");
    expect(resolveLanguage("yml")).toBe("yaml");
    expect(resolveLanguage("py")).toBe("python");
    expect(resolveLanguage("rs")).toBe("rust");
    expect(resolveLanguage("md")).toBe("markdown");
  });

  it("normalises case and whitespace", () => {
    expect(resolveLanguage(" JSON ")).toBe("json");
    expect(resolveLanguage("TypeScript")).toBe("typescript");
  });

  it("maps unknown, empty, and plain labels to text", () => {
    expect(resolveLanguage("klingon")).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage("cobol")).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage("")).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage(undefined)).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage(null)).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage("txt")).toBe(PLAIN_TEXT_LANGUAGE);
    expect(resolveLanguage("plaintext")).toBe(PLAIN_TEXT_LANGUAGE);
  });
});

describe("tokenizeCode", () => {
  it("highlights a curated language into coloured tokens", async () => {
    const result = await tokenizeCode(
      'const answer = 42;\nconsole.log("hi");',
      "javascript"
    );
    expect(result.tokens).toHaveLength(2);
    const [firstLine] = result.tokens;
    expect(firstLine.length).toBeGreaterThan(1);
    // Dual-theme output carries colour in htmlStyle (light) + --shiki-dark.
    const colours = new Set(firstLine.map((token) => token.htmlStyle?.color));
    expect(colours.size).toBeGreaterThan(1);
    expect(firstLine[0].htmlStyle?.["--shiki-dark"]).toBeTruthy();
    expect(firstLine.map((token) => token.content).join("")).toBe(
      "const answer = 42;"
    );
  });

  it("resolves aliases before loading the grammar", async () => {
    const result = await tokenizeCode("echo $HOME", "sh");
    const [line] = result.tokens;
    expect(line.length).toBeGreaterThan(1);
    expect(line.map((token) => token.content).join("")).toBe("echo $HOME");
  });

  it("renders an unknown language as plain text without throwing", async () => {
    const source = "PROGRAM-ID. HELLO.\nDISPLAY 'hi'.";
    const result = await tokenizeCode(source, "cobol");
    expect(result.tokens).toHaveLength(2);
    for (const [index, line] of result.tokens.entries()) {
      expect(line).toHaveLength(1);
      expect(line[0].content).toBe(source.split("\n")[index]);
    }
  });

  it("represents blank lines the same way for plain text and grammars", async () => {
    const plain = await tokenizeCode("a\n\nb", "nope");
    const js = await tokenizeCode("a\n\nb", "js");
    expect(plain.tokens.map((line) => line.length)).toEqual([1, 0, 1]);
    expect(js.tokens.map((line) => line.length)).toEqual([1, 0, 1]);
  });
});

describe("highlightCode", () => {
  it("returns null first, then delivers tokens, then serves from cache", async () => {
    const source = `let x: number = ${Date.now()};`;
    const delivered = await new Promise<
      ReturnType<typeof highlightCode>
    >((resolve) => {
      const immediate = highlightCode(source, "ts", resolve);
      expect(immediate).toBeNull();
    });
    expect(delivered?.tokens[0].length).toBeGreaterThan(1);

    const cached = highlightCode(source, "ts");
    expect(cached).toBe(delivered);
  });

  it("does not throw for an unknown language", async () => {
    const delivered = await new Promise<ReturnType<typeof highlightCode>>(
      (resolve) => {
        expect(() =>
          highlightCode("some text", "definitely-not-a-language", resolve)
        ).not.toThrow();
      }
    );
    expect(delivered?.tokens[0][0].content).toBe("some text");
  });
});

describe("streamdown code plugin", () => {
  it("reports curated languages and aliases as supported", () => {
    expect(streamdownCodePlugin.supportsLanguage("typescript")).toBe(true);
    expect(streamdownCodePlugin.supportsLanguage("yml" as never)).toBe(true);
    expect(streamdownCodePlugin.supportsLanguage("applescript")).toBe(true);
    expect(streamdownCodePlugin.supportsLanguage("cobol")).toBe(false);
    expect(streamdownCodePlugin.getSupportedLanguages()).toEqual(
      SUPPORTED_LANGUAGE_LABELS
    );
  });

  it("uses the same two themes as before", () => {
    expect(streamdownCodePlugin.getThemes()).toEqual([
      "github-light",
      "github-dark",
    ]);
  });

  it("highlights through the shared highlighter", async () => {
    const result = await new Promise<
      ReturnType<typeof streamdownCodePlugin.highlight>
    >((resolve) => {
      streamdownCodePlugin.highlight(
        {
          code: "fn main() {}",
          language: "rust",
          themes: streamdownCodePlugin.getThemes(),
        },
        resolve
      );
    });
    expect(result?.tokens[0].length).toBeGreaterThan(1);
  });
});
