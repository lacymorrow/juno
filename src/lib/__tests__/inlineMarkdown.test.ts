import { describe, expect, it } from "vitest";
import {
  type InlineNode,
  inlineMarkdownToText,
  parseInlineMarkdown,
} from "@/lib/inlineMarkdown";

/** The shape of a tree, in one line, so a test reads as the words it expects. */
function shape(nodes: InlineNode[]): string {
  return nodes
    .map((node) =>
      node.type === "text"
        ? node.value
        : node.type === "code"
          ? `code(${node.value})`
          : `${node.type}(${shape(node.children)})`,
    )
    .join("");
}

const parse = (text: string) => shape(parseInlineMarkdown(text));

describe("parseInlineMarkdown: the marks a preview line can carry", () => {
  it("renders the answer from juno#858 instead of printing its asterisks", () => {
    expect(parse("**Jenny Song**, mobile: 867-5309")).toBe(
      "strong(Jenny Song), mobile: 867-5309",
    );
  });

  it("reads bold, italic, strike and inline code", () => {
    expect(parse("**b**")).toBe("strong(b)");
    expect(parse("__b__")).toBe("strong(b)");
    expect(parse("*i*")).toBe("em(i)");
    expect(parse("_i_")).toBe("em(i)");
    expect(parse("~~gone~~")).toBe("strike(gone)");
    expect(parse("`npm run dev`")).toBe("code(npm run dev)");
  });

  it("nests one mark inside another, in either order", () => {
    expect(parse("*a **b** c*")).toBe("em(a strong(b) c)");
    expect(parse("**a *b* c**")).toBe("strong(a em(b) c)");
    expect(parse("***both***")).toBe("strong(em(both))");
  });

  it("keeps a code span's own delimiters out of the parse", () => {
    expect(parse("`a * b * c`")).toBe("code(a * b * c)");
    expect(parse("`**not bold**`")).toBe("code(**not bold**)");
  });
});

describe("parseInlineMarkdown: prose that only looks like markdown", () => {
  it("leaves arithmetic alone, because a mark cannot open on a space", () => {
    expect(parse("5 * 3 * 2 = 30")).toBe("5 * 3 * 2 = 30");
  });

  it("leaves an identifier alone, because `_` does not open inside a word", () => {
    expect(parse("call snake_case_name twice")).toBe("call snake_case_name twice");
    expect(parse("MAX_RECENT and MIN_RECENT")).toBe("MAX_RECENT and MIN_RECENT");
  });

  it("keeps an unclosed mark as the characters it is", () => {
    expect(parse("**half a thought")).toBe("**half a thought");
    expect(parse("a lone ` backtick")).toBe("a lone ` backtick");
    expect(parse("2 ~ 3")).toBe("2 ~ 3");
  });

  it("honours a backslash escape", () => {
    expect(parse("\\*\\*literally bold\\*\\*")).toBe("**literally bold**");
  });

  it("does not take a phone number or a date for emphasis", () => {
    expect(parse("867-5309 on 2026-10-20")).toBe("867-5309 on 2026-10-20");
  });
});

describe("parseInlineMarkdown: block syntax has no node to land in", () => {
  // The preview is one row. The parser emits inline nodes only, so there is
  // never a paragraph or a list item to grow it; a leading marker is dropped.
  it("drops a heading, quote or bullet that opens the line", () => {
    expect(parse("## Summary")).toBe("Summary");
    expect(parse("> quoted")).toBe("quoted");
    expect(parse("- first")).toBe("first");
    expect(parse("1. first")).toBe("first");
  });

  it("keeps a hyphen that is part of the sentence", () => {
    expect(parse("well-known - and then some")).toBe("well-known - and then some");
  });

  it("shows an image as its alt text, never a stray exclamation mark", () => {
    expect(parse("![The chart](a.png) is ready")).toBe("The chart is ready");
  });

  it("keeps a link's words and its href, for a surface that wants it", () => {
    const nodes = parseInlineMarkdown("see [the docs](https://x.dev/a)");
    expect(shape(nodes)).toBe("see link(the docs)");
    expect(nodes[1]).toMatchObject({ type: "link", href: "https://x.dev/a" });
  });
});

describe("inlineMarkdownToText", () => {
  it("gives the same words with every mark removed", () => {
    expect(inlineMarkdownToText("**Jenny Song**, mobile: 867-5309")).toBe(
      "Jenny Song, mobile: 867-5309",
    );
    expect(inlineMarkdownToText("run `bun test` and [read](x)")).toBe("run bun test and read");
  });
});
