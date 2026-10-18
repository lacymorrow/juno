import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBarChatOpen } from "../useBarChatOpen";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));

const calls = () => vi.mocked(invoke).mock.calls.map(([cmd, args]) => [cmd, args]);

describe("useBarChatOpen", () => {
  beforeEach(() => vi.mocked(invoke).mockClear());

  it("reports the chat opening and closing", () => {
    const { rerender } = renderHook(({ open }) => useBarChatOpen(open), {
      initialProps: { open: false },
    });
    rerender({ open: true });
    rerender({ open: false });
    expect(calls().at(-1)).toEqual(["set_bar_chat_open", { open: false }]);
    expect(calls()).toContainEqual(["set_bar_chat_open", { open: true }]);
  });

  it("reports closed when the look unmounts with its chat open", () => {
    const { unmount } = renderHook(() => useBarChatOpen(true));
    unmount();
    expect(calls().at(-1)).toEqual(["set_bar_chat_open", { open: false }]);
  });
});
