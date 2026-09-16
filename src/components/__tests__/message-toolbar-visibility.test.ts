import { describe, expect, it } from "vitest";

import { isWorthExporting } from "@/components/ChatMessageV2";
import type { ChatMessage } from "@/types/chat";

function assistant(content: string, extra: Partial<ChatMessage> = {}): ChatMessage {
  return { role: "assistant", content, ...extra };
}

describe("isWorthExporting", () => {
  it("offers to export an ordinary reply", () => {
    expect(isWorthExporting(assistant("Here is what I found."))).toBe(true);
  });

  it("offers to export a generated component, whose words do get copied", () => {
    expect(
      isWorthExporting(assistant('<WeatherCard temp="64" summary="Clear" />'))
    ).toBe(true);
  });

  it("leaves onboarding notices alone", () => {
    // The whole onboarding transcript streams through the same pipeline as a
    // real reply, so without the flag every step of setup grew a Copy button.
    expect(
      isWorthExporting(assistant("Setup complete. Welcome to Juno!", { notice: true }))
    ).toBe(false);
  });

  it("leaves a run that only explains itself alone", () => {
    // A delegated task that spoke its answer and collapsed its method into a
    // Why block has nothing on screen worth keeping.
    expect(
      isWorthExporting(assistant("<Why>I used the desktop agent because …</Why>"))
    ).toBe(false);
  });

  it("still exports a reply that has prose alongside its rationale", () => {
    expect(
      isWorthExporting(
        assistant("I moved the file to Documents.\n<Why>It was the only match.</Why>")
      )
    ).toBe(true);
  });

  it("leaves a spoken-only turn alone", () => {
    expect(isWorthExporting(assistant("<TTS>All done.</TTS>"))).toBe(false);
  });

  it("waits until the reply has finished arriving", () => {
    expect(isWorthExporting(assistant("Here is what", { isStreaming: true }))).toBe(false);
  });

  it("ignores everything that is not an assistant reply", () => {
    expect(isWorthExporting({ role: "user", content: "hello" })).toBe(false);
    expect(isWorthExporting({ role: "system", content: "Connected." })).toBe(false);
    expect(
      isWorthExporting({ role: "tool_call_result", content: "Completed" })
    ).toBe(false);
  });

  it("ignores an empty reply", () => {
    expect(isWorthExporting(assistant("   "))).toBe(false);
  });
});
