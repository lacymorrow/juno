import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UI } from "@/lib/constants.generated";
import { APPEARANCE_CATALOG, appearanceEntry } from "../appearanceCatalog";

/**
 * Restoring the Orb renamed the look a person already had: the ElevenLabs orb
 * is now called Presence, and the name "Orb" moved to the restored React Bits
 * shader, which has a value of its own.
 *
 * A rename must never move anyone to a different look. These tests pin the
 * stored value to the component it renders, so a future edit that quietly
 * repoints `orb` or `react_orb` fails here instead of on someone's desk.
 */

const { invoke, listen, stub } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(null)),
  listen: vi.fn(async () => () => {}),
  // Every look is replaced by a marker, so this test is about which component
  // the stored value reaches and nothing else.
  stub: (name: string) => () => <div data-testid="look">{name}</div>,
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen, emit: vi.fn(async () => {}) }));

vi.mock("@/components/FloatingBar", () => ({ FloatingBar: stub("pill") }));
vi.mock("@/components/bar/app-bar", () => ({ AppBar: stub("bar") }));
vi.mock("@/components/bar/island/IslandBar", () => ({ IslandBar: stub("island") }));
vi.mock("@/components/bar/voice-ai-bar", () => ({ VoiceAIBar: stub("studio") }));
vi.mock("@/components/bar/elevenlabs-orb-bar", () => ({ ElevenLabsOrbBar: stub("elevenlabs-orb") }));
vi.mock("@/components/bar/react-orb-bar", () => ({ ReactOrbBar: stub("halo-ring") }));
vi.mock("@/components/bar/shader-orb-bar", () => ({ ShaderOrbBar: stub("react-bits-orb") }));
vi.mock("@/components/bar/persona-bar", () => ({ PersonaBar: stub("avatar") }));

import { BarHost } from "../BarHost";

/** What a person has in their settings store, and what they must still see. */
const STORED: readonly { value: string; name: string; look: string }[] = [
  { value: "floating", name: "Pill", look: "pill" },
  { value: "app", name: "Bar", look: "bar" },
  { value: "voice_ai", name: "Studio", look: "studio" },
  { value: "dynamic", name: "Island", look: "island" },
  { value: "shader_orb", name: "Orb", look: "react-bits-orb" },
  { value: "orb", name: "Presence", look: "elevenlabs-orb" },
  { value: "react_orb", name: "Halo", look: "halo-ring" },
  { value: "persona", name: "Avatar", look: "avatar" },
];

async function mount(stored: string | null) {
  invoke.mockResolvedValue({
    show_voice_indicator: true,
    enable_animations: true,
    auto_hide: false,
    auto_hide_delay: 3000,
    opacity: 0.95,
    bar_appearance: stored,
    show_glow_border: true,
  });
  render(<BarHost />);
  // The config arrives, then the lazy chunk resolves.
  for (let i = 0; i < 4; i += 1) await act(async () => {});
}

beforeEach(() => {
  invoke.mockClear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("a stored appearance after the rename", () => {
  it("covers every value the catalog offers, and no more", () => {
    expect(APPEARANCE_CATALOG.map((e) => e.value)).toEqual(STORED.map((s) => s.value));
  });

  it("shows each one the name it is meant to have", () => {
    for (const { value, name } of STORED) {
      expect(appearanceEntry(value).name, value).toBe(name);
    }
  });

  it.each(STORED)("leaves someone stored on $value looking at $look", async ({ value, look }) => {
    await mount(value);
    expect(screen.getByTestId("look").textContent).toBe(look);
  });

  it("keeps the ElevenLabs orb for anyone stored on 'orb', renamed and not replaced", async () => {
    await mount("orb");
    expect(screen.getByTestId("look").textContent).toBe("elevenlabs-orb");
    expect(appearanceEntry("orb").name).toBe("Presence");
  });

  it("gives the restored orb a value of its own, so neither old one was taken", () => {
    const restored = appearanceEntry(UI.BAR_APPEARANCES_SHADER_ORB);
    expect(restored.name).toBe("Orb");
    expect(restored.value).not.toBe(UI.BAR_APPEARANCES_ORB);
    expect(restored.value).not.toBe(UI.BAR_APPEARANCES_REACT_ORB);
  });

  it("never gives two looks the same value or the same name", () => {
    const values = APPEARANCE_CATALOG.map((e) => e.value);
    const names = APPEARANCE_CATALOG.map((e) => e.name);
    expect(new Set(values).size).toBe(values.length);
    expect(new Set(names).size).toBe(names.length);
  });

  it("falls back to the default look for a value no release ever wrote", async () => {
    expect(appearanceEntry("halo").value).toBe(UI.BAR_APPEARANCES_FLOATING);
    expect(appearanceEntry(null).value).toBe(UI.BAR_APPEARANCES_FLOATING);
    await mount("react_bits_orb");
    expect(screen.getByTestId("look").textContent).toBe("pill");
  });
});
