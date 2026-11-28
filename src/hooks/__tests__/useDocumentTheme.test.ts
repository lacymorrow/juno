import { renderHook, waitFor, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDocumentTheme } from "../useSystemTheme";

let emit: ((e: { payload: string }) => void) | undefined;
const unlisten = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    theme: () => Promise.resolve("dark"),
    onThemeChanged: (handler: (e: { payload: string }) => void) => {
      emit = handler;
      return Promise.resolve(unlisten);
    },
  }),
}));

const isDark = () => document.documentElement.classList.contains("dark");

describe("useDocumentTheme", () => {
  beforeEach(() => {
    emit = undefined;
    unlisten.mockClear();
  });

  it("follows the OS appearance on <html>, live, and cleans up", async () => {
    const { unmount } = renderHook(() => useDocumentTheme());

    await waitFor(() => expect(isDark()).toBe(true));
    await waitFor(() => expect(emit).toBeDefined());

    act(() => emit!({ payload: "light" }));
    expect(isDark()).toBe(false);

    act(() => emit!({ payload: "dark" }));
    expect(isDark()).toBe(true);

    unmount();
    expect(isDark()).toBe(false);
    expect(unlisten).toHaveBeenCalled();
  });
});
