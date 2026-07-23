import { describe, expect, it } from "vitest";
import { fuzzyMatch, rankFuzzy } from "../fuzzy-match";

describe("fuzzyMatch", () => {
  it("matches empty query against anything with score 0", () => {
    expect(fuzzyMatch("", "paperclip")).toEqual({ score: 0, positions: [] });
  });

  it("returns null when query is not a subsequence", () => {
    expect(fuzzyMatch("xyz", "paperclip")).toBeNull();
    expect(fuzzyMatch("paperclips", "paperclip")).toBeNull();
  });

  it("ranks exact above prefix above subsequence", () => {
    const exact = fuzzyMatch("paperclip", "paperclip");
    const prefix = fuzzyMatch("pap", "paperclip");
    const subseq = fuzzyMatch("ppc", "paperclip");
    expect(exact).not.toBeNull();
    expect(prefix).not.toBeNull();
    expect(subseq).not.toBeNull();
    expect(exact!.score).toBeGreaterThan(prefix!.score);
    expect(prefix!.score).toBeGreaterThan(subseq!.score);
  });

  it("is case-insensitive", () => {
    expect(fuzzyMatch("PAP", "paperclip")).not.toBeNull();
    expect(fuzzyMatch("pap", "PaperClip")).not.toBeNull();
  });

  it("reports matched positions for prefix matches", () => {
    expect(fuzzyMatch("pap", "paperclip")!.positions).toEqual([0, 1, 2]);
  });

  it("prefers shorter names among equal prefix matches", () => {
    const short = fuzzyMatch("pap", "paperclip")!;
    const long = fuzzyMatch("pap", "paperclip-board-tools")!;
    expect(short.score).toBeGreaterThan(long.score);
  });

  it("rewards word-boundary matches", () => {
    // "sr" hits s of "security" and r of "review" (boundary after "-")
    const boundary = fuzzyMatch("sr", "security-review")!;
    const buried = fuzzyMatch("sr", "userland")!;
    expect(boundary.score).toBeGreaterThan(buried.score);
  });
});

describe("rankFuzzy", () => {
  const skills = [
    { name: "paperclip" },
    { name: "paperclip-board" },
    { name: "pdf" },
    { name: "polish" },
    { name: "security-review" },
  ];

  it("returns matches sorted by score and drops non-matches", () => {
    const ranked = rankFuzzy("pap", skills, (s) => s.name);
    expect(ranked.map((r) => r.item.name)).toEqual([
      "paperclip",
      "paperclip-board",
    ]);
  });

  it("empty query returns everything", () => {
    expect(rankFuzzy("", skills, (s) => s.name)).toHaveLength(skills.length);
  });

  it("fuzzy subsequence still matches", () => {
    const ranked = rankFuzzy("secrev", skills, (s) => s.name);
    expect(ranked[0]?.item.name).toBe("security-review");
  });
});
