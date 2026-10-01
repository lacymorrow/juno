import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { JunoVoiceOption } from "@/hooks/useSettings";
import { VoicePicker } from "../VoicePicker";

/** The shape Rust hands over: silence first, then the curated voices. */
function options(selectedId = "Samantha"): JunoVoiceOption[] {
  return [
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
      descriptor: "American English. Juno's default.",
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
  ];
}

function renderPicker(selectedId = "Samantha") {
  const onChange = vi.fn();
  const onReplay = vi.fn();
  render(
    <VoicePicker
      options={options(selectedId)}
      onChange={onChange}
      onReplay={onReplay}
      speakingId={null}
    />,
  );
  return { onChange, onReplay };
}

describe("VoicePicker", () => {
  /// The whole design: choosing is how you hear it. The handler that saves the
  /// choice is the handler that speaks, so a row tap must reach it.
  it("picking a voice is what plays it", () => {
    const { onChange, onReplay } = renderPicker();
    fireEvent.click(screen.getByRole("radio", { name: /Daniel/ }));
    expect(onChange).toHaveBeenCalledWith("Daniel");
    expect(onReplay).not.toHaveBeenCalled();
  });

  /// Tapping the voice already in force is not a no-op: it is "say that again".
  it("picking the voice already chosen plays it again instead of re-saving", () => {
    const { onChange, onReplay } = renderPicker();
    fireEvent.click(screen.getByRole("radio", { name: /Samantha/ }));
    expect(onReplay).toHaveBeenCalledTimes(1);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("exactly one row is in force", () => {
    renderPicker("Daniel");
    const checked = screen
      .getAllByRole("radio")
      .filter((row) => row.getAttribute("aria-checked") === "true");
    expect(checked).toHaveLength(1);
    expect(checked[0]).toHaveTextContent("Daniel");
  });

  /// Every row carries a name and one honest line, the way the appearance
  /// catalog does. A name on its own is not a choice.
  it("every row shows a name and one line", () => {
    renderPicker();
    for (const option of options()) {
      expect(screen.getByText(option.name)).toBeInTheDocument();
      expect(screen.getByText(option.descriptor)).toBeInTheDocument();
    }
  });

  /// Silence has nothing to audition, so it must not offer to play.
  it("silence does not offer a sound", () => {
    renderPicker();
    const silent = screen.getByRole("radio", { name: /Silent/ });
    expect(silent.querySelector("svg.lucide-volume2")).toBeNull();
  });

  it("the speaking row says it is speaking", () => {
    render(
      <VoicePicker
        options={options()}
        onChange={vi.fn()}
        onReplay={vi.fn()}
        speakingId="Samantha"
      />,
    );
    expect(screen.getByRole("radio", { name: /Samantha/ })).toHaveTextContent("Speaking");
    expect(screen.getByRole("radio", { name: /Daniel/ })).not.toHaveTextContent("Speaking");
  });

  /// An empty list is a designed state, not a blank card.
  it("says so when there are no voices to offer", () => {
    render(
      <VoicePicker options={[]} onChange={vi.fn()} onReplay={vi.fn()} speakingId={null} />,
    );
    expect(screen.getByText(/could not read this Mac's voices/)).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });
});
