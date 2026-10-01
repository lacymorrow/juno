import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { JunoVoiceList, VoiceAudition } from "@/hooks/useSettings";
import { VoicePicker } from "../VoicePicker";

/** The shape Rust hands over when the Mac is speaking. */
function macList(selectedId = "Samantha", overrides: Partial<JunoVoiceList> = {}): JunoVoiceList {
  return {
    provider: "system",
    engine: "system",
    engine_label: "Your Mac",
    note: null,
    better_voices_available: false,
    options: [
      {
        id: "silent",
        kind: "silent",
        name: "Silent",
        descriptor: "Juno writes the answer and never says it out loud.",
        selected: selectedId === "silent",
        speaks: false,
      },
      {
        id: "Samantha",
        kind: "voice",
        name: "Samantha",
        descriptor: "American English. On every Mac.",
        selected: selectedId === "Samantha",
        speaks: true,
      },
      {
        id: "Daniel",
        kind: "voice",
        name: "Daniel",
        descriptor: "British English.",
        selected: selectedId === "Daniel",
        speaks: true,
      },
    ],
    ...overrides,
  };
}

/** The shape Rust hands over when Kokoro is speaking. */
function kokoroList(): JunoVoiceList {
  return {
    provider: "kokoro",
    engine: "kokoro",
    engine_label: "Kokoro",
    note: null,
    better_voices_available: false,
    options: [
      {
        id: "silent",
        kind: "silent",
        name: "Silent",
        descriptor: "Juno writes the answer and never says it out loud.",
        selected: false,
        speaks: false,
      },
      {
        id: "af_heart",
        kind: "voice",
        name: "Heart",
        descriptor: "American English, female.",
        selected: true,
        speaks: true,
      },
    ],
  };
}

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
    const { onChange, onReplay } = renderPicker(macList());
    fireEvent.click(screen.getByRole("radio", { name: /Daniel/ }));
    expect(onChange).toHaveBeenCalledWith("Daniel");
    expect(onReplay).not.toHaveBeenCalled();
  });

  /// Tapping the voice already in force is not a no-op: it is "say that again".
  it("picking the voice already chosen plays it again instead of re-saving", () => {
    const { onChange, onReplay } = renderPicker(macList());
    fireEvent.click(screen.getByRole("radio", { name: /Samantha/ }));
    expect(onReplay).toHaveBeenCalledTimes(1);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("exactly one row is in force", () => {
    renderPicker(macList("Daniel"));
    const checked = screen
      .getAllByRole("radio")
      .filter((row) => row.getAttribute("aria-checked") === "true");
    expect(checked).toHaveLength(1);
    expect(checked[0]).toHaveTextContent("Daniel");
  });

  /// Every row carries a name and one honest line, the way the appearance
  /// catalog does. A name on its own is not a choice.
  it("every row shows a name and one line", () => {
    renderPicker(macList());
    for (const option of macList().options) {
      expect(screen.getByText(option.name)).toBeInTheDocument();
      expect(screen.getByText(option.descriptor)).toBeInTheDocument();
    }
  });

  /// The defect this component was rewritten for: the rows are the active
  /// engine's, and nothing here invents a list of its own. Give it Kokoro's
  /// voices and Kokoro's voices are what it draws.
  it("draws whichever engine's voices it was given", () => {
    renderPicker(kokoroList());
    expect(screen.getByRole("radio", { name: /Heart/ })).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: /Samantha/ })).toBeNull();
    expect(screen.queryByRole("radio", { name: /Daniel/ })).toBeNull();
  });

  /// Silence has nothing to audition, so it must not offer to play.
  it("silence does not offer a sound", () => {
    renderPicker(macList());
    const silent = screen.getByRole("radio", { name: /Silent/ });
    expect(silent.querySelector("svg.lucide-volume2")).toBeNull();
  });

  it("the speaking row says it is speaking", () => {
    renderPicker(macList(), {
      voice: "Samantha",
      engine: "system",
      state: "speaking",
      message: null,
    });
    expect(screen.getByRole("radio", { name: /Samantha/ })).toHaveTextContent("Speaking");
    expect(screen.getByRole("radio", { name: /Daniel/ })).not.toHaveTextContent("Speaking");
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
      macList("silent", {
        provider: "elevenlabs",
        engine: "elevenlabs",
        engine_label: "ElevenLabs",
        note: "ElevenLabs speaks with the voice set on your ElevenLabs account, not here.",
        options: [
          {
            id: "silent",
            kind: "silent",
            name: "Silent",
            descriptor: "Juno writes the answer and never says it out loud.",
            selected: false,
            speaks: false,
          },
        ],
      }),
    );
    expect(screen.getByText(/not here/)).toBeInTheDocument();
    expect(screen.getAllByRole("radio")).toHaveLength(1);
  });

  /// Waiting on Rust is a designed state, not a blank card.
  it("says so before Rust has answered", () => {
    renderPicker(null);
    expect(screen.getByText(/Reading this Mac's voices/)).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });
});
