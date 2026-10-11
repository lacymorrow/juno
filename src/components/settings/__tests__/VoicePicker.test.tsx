import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));

import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import type { JunoVoiceList, VoiceAudition } from "@/hooks/useSettings";
import { VoicePicker } from "../VoicePicker";

const invokeMock = vi.mocked(invoke);

/**
 * The shape Rust hands over when the Mac is speaking: one row, the Mac's own
 * voice (Siri, set in Read & Speak), never a named Mac voice.
 */
function macList(descriptor = "Uses the Siri voice you pick in Read & Speak."): JunoVoiceList {
  return {
    provider: "system",
    silent: false,
    engine: "system",
    engine_label: "Your Mac",
    note: null,
    engines: [],
    options: [
      {
        id: "system_default",
        kind: "system",
        name: "Your Mac's voice",
        descriptor,
        selected: true,
        speaks: true,
        link: "Open Read & Speak",
      },
    ],
  };
}

/** The shape Rust hands over when Kokoro is speaking. */
function kokoroList(selectedId = "af_heart", overrides: Partial<JunoVoiceList> = {}): JunoVoiceList {
  return {
    provider: "kokoro",
    silent: false,
    engine: "kokoro",
    engine_label: "Kokoro",
    note: null,
    engines: [],
    options: [
      {
        id: "af_heart",
        kind: "voice",
        name: "Heart",
        descriptor: "American English, female.",
        selected: selectedId === "af_heart",
        speaks: true,
      },
      {
        id: "am_michael",
        kind: "voice",
        name: "Michael",
        descriptor: "American English, male.",
        selected: selectedId === "am_michael",
        speaks: true,
      },
    ],
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockClear();
});

function renderPicker(list: JunoVoiceList | null, audition: VoiceAudition | null = null) {
  const onChange = vi.fn();
  const onReplay = vi.fn();
  render(
    <VoicePicker list={list} onChange={onChange} onReplay={onReplay} audition={audition} />,
  );
  return { onChange, onReplay };
}

describe("VoicePicker", () => {
  /// The whole design: choosing is how you hear it. The handler that saves the
  /// choice is the handler that speaks, so a row tap must reach it.
  it("picking a voice is what plays it", () => {
    const { onChange, onReplay } = renderPicker(kokoroList());
    fireEvent.click(screen.getByRole("radio", { name: /Michael/ }));
    expect(onChange).toHaveBeenCalledWith("am_michael");
    expect(onReplay).not.toHaveBeenCalled();
  });

  /// Tapping the voice already in force is not a no-op: it is "say that again".
  it("picking the voice already chosen plays it again instead of re-saving", () => {
    const { onChange, onReplay } = renderPicker(kokoroList());
    fireEvent.click(screen.getByRole("radio", { name: /Heart/ }));
    expect(onReplay).toHaveBeenCalledTimes(1);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("exactly one row is in force", () => {
    renderPicker(kokoroList("am_michael"));
    const checked = screen
      .getAllByRole("radio")
      .filter((row) => row.getAttribute("aria-checked") === "true");
    expect(checked).toHaveLength(1);
    expect(checked[0]).toHaveTextContent("Michael");
  });

  /// Every row carries a name and one honest line, the way the appearance
  /// catalog does. A name on its own is not a choice.
  it("every row shows a name and one line", () => {
    renderPicker(kokoroList());
    for (const option of kokoroList().options) {
      expect(screen.getByText(option.name)).toBeInTheDocument();
      expect(screen.getByText(option.descriptor)).toBeInTheDocument();
    }
  });

  /// The Mac offers its own voice and nothing else: Juno never speaks in a
  /// named Mac voice, only the Siri voice chosen in Read & Speak.
  it("the Mac is one row, its own voice", () => {
    renderPicker(macList());
    const rows = screen.getAllByRole("radio");
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("Your Mac's voice");
    expect(rows[0]).toHaveTextContent("Uses the Siri voice you pick in Read & Speak.");
  });

  /// Where the Mac's voice is set is one tap away, and Rust opens it.
  it("the Mac row opens Read & Speak through Rust, without picking", () => {
    const { onChange, onReplay } = renderPicker(macList());
    fireEvent.click(screen.getByRole("button", { name: "Open Read & Speak" }));
    expect(invokeMock).toHaveBeenCalledWith(COMMANDS.AUDIO_OPEN_READ_AND_SPEAK);
    expect(onChange).not.toHaveBeenCalled();
    expect(onReplay).not.toHaveBeenCalled();
  });

  /// When the Mac's voice is not a Siri voice the row says how to fix it,
  /// in the words Rust chose.
  it("says how to pick a Siri voice when the Mac's is not one", () => {
    renderPicker(macList("Pick a Siri voice in Read & Speak and Juno will use it."));
    expect(
      screen.getByText("Pick a Siri voice in Read & Speak and Juno will use it."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Read & Speak" })).toBeInTheDocument();
  });

  /// A row with no link draws none.
  it("only a row Rust gave a link has one", () => {
    renderPicker(kokoroList());
    expect(screen.queryByRole("button", { name: "Open Read & Speak" })).toBeNull();
  });

  /// The rows are the active engine's, and nothing here invents a list of
  /// its own. Give it Kokoro's voices and Kokoro's voices are what it draws.
  it("draws whichever engine's voices it was given", () => {
    renderPicker(kokoroList());
    expect(screen.getByRole("radio", { name: /Heart/ })).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: /Your Mac's voice/ })).toBeNull();
  });

  it("the speaking row says it is speaking", () => {
    renderPicker(kokoroList(), {
      voice: "af_heart",
      engine: "kokoro",
      state: "speaking",
      message: null,
    });
    expect(screen.getByRole("radio", { name: /Heart/ })).toHaveTextContent("Speaking");
    expect(screen.getByRole("radio", { name: /Michael/ })).not.toHaveTextContent("Speaking");
  });

  /// An engine that loads a model takes seconds before a sound comes out.
  /// Saying "Speaking" through that would be a lie and saying nothing is what
  /// reads as broken, so there is a third state and it is drawn.
  it("a sample that has not started yet says it is loading", () => {
    renderPicker(kokoroList(), {
      voice: "af_heart",
      engine: "kokoro",
      state: "preparing",
      message: null,
    });
    const row = screen.getByRole("radio", { name: /Heart/ });
    expect(row).toHaveTextContent("Loading");
    expect(row).not.toHaveTextContent("Speaking");
  });

  /// A sample that could not play says why, in the words Rust chose.
  it("a failed sample shows the reason", () => {
    renderPicker(kokoroList(), {
      voice: "af_heart",
      engine: "kokoro",
      state: "failed",
      message: "Kokoro could not speak the sample: no network.",
    });
    expect(screen.getByText(/Kokoro could not speak the sample: no network./)).toBeInTheDocument();
  });

  /// An engine whose voices are chosen elsewhere says so instead of offering
  /// rows it cannot honour.
  it("shows the note when there is something to say instead of rows", () => {
    renderPicker(
      kokoroList("none", {
        provider: "elevenlabs",
        engine: "elevenlabs",
        engine_label: "ElevenLabs",
        note: "ElevenLabs speaks with the voice set on your ElevenLabs account, not here.",
        options: [],
      }),
    );
    expect(screen.getByText(/not here/)).toBeInTheDocument();
    expect(screen.queryAllByRole("radio")).toHaveLength(0);
  });

  /// Waiting on Rust is a designed state, not a blank card.
  it("says so before Rust has answered", () => {
    renderPicker(null);
    expect(screen.getByText(/Reading this Mac's voices/)).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });
});
