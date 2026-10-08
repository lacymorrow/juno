import { describe, expect, it } from "vitest";

import { UI } from "@/lib/constants.generated";
import { DOT_COLORS, dotLabel, dotTone, type Connectivity } from "@/lib/pillStatus";
import { hintDetail, statusDotColor } from "@/components/FloatingBar";

const connected: Connectivity = { status: "connected", label: "Connected to Claude", provider: "Claude" };
const offline: Connectivity = { status: "offline", label: "No internet connection", provider: "Claude" };
const unreachable: Connectivity = {
  status: "provider_unreachable",
  label: "Can't reach Claude",
  provider: "Claude",
};
const signedOut: Connectivity = { status: "signed_out", label: "Not signed in to Claude", provider: "Claude" };

describe("dotTone", () => {
  it("is neutral at rest and while working, whatever the bar is doing", () => {
    for (const state of [UI.BAR_STATES_DEFAULT, UI.BAR_STATES_LOADING, UI.BAR_STATES_SPEAKING, UI.BAR_STATES_ERROR]) {
      expect(dotTone(state, connected)).toBe("neutral");
    }
    // Before Rust has answered, the dot assumes the best.
    expect(dotTone(UI.BAR_STATES_DEFAULT, null)).toBe("neutral");
  });

  it("is red for every way Juno cannot answer", () => {
    for (const c of [offline, unreachable, signedOut]) {
      expect(dotTone(UI.BAR_STATES_DEFAULT, c)).toBe("down");
      expect(dotTone(UI.BAR_STATES_LOADING, c)).toBe("down");
    }
  });

  it("is blue whenever a microphone is open, connected or not", () => {
    for (const state of [UI.BAR_STATES_LISTENING, UI.BAR_STATES_ALWAYS_LISTENING, UI.BAR_STATES_DICTATING]) {
      expect(dotTone(state, connected)).toBe("listening");
      expect(dotTone(state, offline)).toBe("listening");
    }
  });
});

describe("dotTone while Juno is still loading at launch", () => {
  it("pulses blue at rest", () => {
    expect(dotTone(UI.BAR_STATES_DEFAULT, connected, true)).toBe("loading");
    expect(dotTone(UI.BAR_STATES_DEFAULT, null, true)).toBe("loading");
  });

  it("gives way to red, to an open microphone, and to work in progress", () => {
    expect(dotTone(UI.BAR_STATES_DEFAULT, offline, true)).toBe("down");
    expect(dotTone(UI.BAR_STATES_LISTENING, connected, true)).toBe("listening");
    expect(dotTone(UI.BAR_STATES_DICTATING, offline, true)).toBe("listening");
    for (const state of [UI.BAR_STATES_LOADING, UI.BAR_STATES_SPEAKING, UI.BAR_STATES_TRANSCRIBING]) {
      expect(dotTone(state, connected, true)).toBe("neutral");
    }
  });

  it("says so in the tooltip, only at rest", () => {
    expect(dotLabel(UI.BAR_STATES_DEFAULT, connected, { loading: true })).toBe("Getting ready");
    expect(dotLabel(UI.BAR_STATES_DEFAULT, offline, { loading: true })).toBe("No internet connection");
    expect(dotLabel(UI.BAR_STATES_LOADING, connected, { loading: true })).toBe("Working");
  });

  it("is system blue, the one accent", () => {
    expect(statusDotColor(UI.BAR_STATES_DEFAULT, connected, { loading: true })).toBe("#0A84FF");
  });
});

describe("statusDotColor", () => {
  it("uses system blue and system red, and no other accent", () => {
    expect(statusDotColor(UI.BAR_STATES_LISTENING, connected)).toBe(DOT_COLORS.listening);
    expect(statusDotColor(UI.BAR_STATES_DEFAULT, offline)).toBe(DOT_COLORS.down);
    expect(DOT_COLORS).toEqual({ listening: "#0A84FF", down: "#FF453A", loading: "#0A84FF" });
    // States that used to carry their own colours are neutral now.
    for (const state of [UI.BAR_STATES_ERROR, UI.BAR_STATES_SUCCESS, UI.BAR_STATES_DICTATION_READY]) {
      expect(statusDotColor(state, connected)).toMatch(/^(#ffffff|rgba\(255,255,255,[\d.]+\))$/);
    }
  });

  it("is system blue while Juno has the pointer", () => {
    expect(statusDotColor(UI.BAR_STATES_LOADING, offline, { driving: true })).toBe("#0A84FF");
  });
});

describe("dotLabel", () => {
  it("says what the dot shows", () => {
    expect(dotLabel(UI.BAR_STATES_DEFAULT, connected)).toBe("Connected to Claude");
    expect(dotLabel(UI.BAR_STATES_DEFAULT, null)).toBe("Connected");
    expect(dotLabel(UI.BAR_STATES_DEFAULT, offline)).toBe("No internet connection");
    expect(dotLabel(UI.BAR_STATES_LOADING, unreachable)).toBe("Can't reach Claude");
    expect(dotLabel(UI.BAR_STATES_LISTENING, offline)).toBe("Listening");
    expect(dotLabel(UI.BAR_STATES_DICTATING, connected)).toBe("Dictating");
    expect(dotLabel(UI.BAR_STATES_LOADING, connected)).toBe("Working");
    expect(dotLabel(UI.BAR_STATES_DEFAULT, connected, { voicePaused: true })).toBe(
      "Connected to Claude, wake phrase paused",
    );
    expect(dotLabel(UI.BAR_STATES_DEFAULT, connected, { driving: true })).toBe(
      "Juno is using the pointer",
    );
  });
});

describe("hintDetail", () => {
  it("draws the gesture and the key caps, or nothing when no key is bound", () => {
    expect(hintDetail({ shortcut: "Control+Option", gesture: "Hold" })).toBe("Hold ⌃⌥");
    expect(hintDetail({ shortcut: " ", gesture: "Hold" })).toBeNull();
    expect(hintDetail(null)).toBeNull();
  });
});
