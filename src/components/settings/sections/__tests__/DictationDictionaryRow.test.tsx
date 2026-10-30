import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import {
  DICTIONARY_EMPTY,
  DictationDictionaryRow,
} from "../DictationDictionaryRow";

const invokeMock = vi.mocked(invoke);

/** The backend: answers the GET with `initial`, echoes every SET tidied. */
function backend(initial: string[]) {
  invokeMock.mockImplementation(((command: string, args?: unknown) => {
    if (command === COMMANDS.DICTATION_GET_DICTATION_DICTIONARY) {
      return Promise.resolve(initial);
    }
    if (command === COMMANDS.DICTATION_SET_DICTATION_DICTIONARY) {
      const { words } = args as { words: string[] };
      return Promise.resolve(words.map((w) => w.trim()).filter(Boolean));
    }
    return Promise.resolve(undefined);
  }) as typeof invoke);
}

describe("DictationDictionaryRow", () => {
  beforeEach(() => invokeMock.mockReset());

  it("draws the saved words as chips", async () => {
    backend(["Juno", "Lacy"]);
    render(<DictationDictionaryRow />);
    expect(await screen.findByText("Juno")).toBeInTheDocument();
    expect(screen.getByText("Lacy")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove Juno" })).toBeInTheDocument();
  });

  it("adds a word on Return and saves the whole list", async () => {
    backend(["Juno"]);
    render(<DictationDictionaryRow />);
    const input = await screen.findByPlaceholderText("Add a word, press Return");
    fireEvent.change(input, { target: { value: "  Paperclip " } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.DICTATION_SET_DICTATION_DICTIONARY,
        { words: ["Juno", "Paperclip"] },
      ),
    );
    expect(await screen.findByText("Paperclip")).toBeInTheDocument();
    expect(input).toHaveValue("");
  });

  it("ignores a repeat in any case and a blank", async () => {
    backend(["Juno"]);
    render(<DictationDictionaryRow />);
    const input = await screen.findByPlaceholderText("Add a word, press Return");
    fireEvent.change(input, { target: { value: "juno" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(invokeMock).not.toHaveBeenCalledWith(
      COMMANDS.DICTATION_SET_DICTATION_DICTIONARY,
      expect.anything(),
    );
  });

  it("removes a word from its x and shows the empty state when none are left", async () => {
    backend(["Juno"]);
    render(<DictationDictionaryRow />);
    fireEvent.click(await screen.findByRole("button", { name: "Remove Juno" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.DICTATION_SET_DICTATION_DICTIONARY,
        { words: [] },
      ),
    );
    expect(await screen.findByText(DICTIONARY_EMPTY)).toBeInTheDocument();
  });

  it("puts the list back when the save fails", async () => {
    backend(["Juno"]);
    render(<DictationDictionaryRow />);
    await screen.findByText("Juno");
    invokeMock.mockImplementation(((command: string) =>
      command === COMMANDS.DICTATION_SET_DICTATION_DICTIONARY
        ? Promise.reject(new Error("store closed"))
        : Promise.resolve(["Juno"])) as typeof invoke);
    fireEvent.click(screen.getByRole("button", { name: "Remove Juno" }));
    await waitFor(() => expect(screen.getByText("Juno")).toBeInTheDocument());
  });

  it("renders nothing when the backend has no list to give", async () => {
    invokeMock.mockImplementation((() => Promise.resolve(undefined)) as typeof invoke);
    const { container } = render(<DictationDictionaryRow />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});
