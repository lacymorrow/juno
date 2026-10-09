import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { checkLink, isJunoLink, junoUrlTransform } from "../junoLinks";

describe("juno:// links in replies", () => {
  beforeEach(() => invoke.mockClear());

  it("recognises the scheme in any case", () => {
    expect(isJunoLink("juno://settings/voice")).toBe(true);
    expect(isJunoLink("JUNO://settings")).toBe(true);
    expect(isJunoLink("https://junebug.ai")).toBe(false);
  });

  it("keeps a juno link through the markdown url check", () => {
    expect(junoUrlTransform("juno://settings/voice", "href", {} as never)).toBe(
      "juno://settings/voice",
    );
  });

  it("hands a juno link to Rust and never lets the library open it", () => {
    const result = checkLink("juno://settings/voice");
    expect(invoke).toHaveBeenCalledWith("open_juno_link", {
      url: "juno://settings/voice",
    });
    expect(result).toBeInstanceOf(Promise);
  });

  it("leaves other links to the library's own confirmation", () => {
    expect(checkLink("https://junebug.ai")).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });
});
