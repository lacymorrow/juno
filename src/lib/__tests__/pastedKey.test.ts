import { describe, expect, it } from "vitest";

import { findPastedKey } from "@/lib/pastedKey";

const ANT = "sk-ant-api03-" + "a1B2".repeat(10);
const OAI = "sk-proj-" + "c3D4".repeat(10);
const GEM = "AIza" + "e5F6".repeat(9);

describe("findPastedKey", () => {
  it("reads a bare key from each provider", () => {
    expect(findPastedKey(ANT)).toEqual({ key: ANT, provider: { id: "anthropic", name: "Anthropic" } });
    expect(findPastedKey(OAI)?.provider.id).toBe("openai");
    expect(findPastedKey(GEM)?.provider.id).toBe("gemini");
  });

  it("finds the key inside the message it came in", () => {
    const message = `Hey! Here's your key for Juno:\n\n  "${ANT}".\n\nDon't share it. Lacy`;
    expect(findPastedKey(message)?.key).toBe(ANT);
  });

  it("finds nothing in ordinary text", () => {
    expect(findPastedKey("")).toBeNull();
    expect(findPastedKey("my password is hunter2, ask-me later")).toBeNull();
    expect(findPastedKey("sk-short")).toBeNull();
  });

  it("does not mistake an ordinary word for a key", () => {
    expect(findPastedKey("the risk-assessment-for-the-project is attached")).toBeNull();
    expect(findPastedKey("task-force-alpha-bravo-charlie")).toBeNull();
    expect(findPastedKey("xAIza" + "e5F6".repeat(9))).toBeNull();
  });
});
