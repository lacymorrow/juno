import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import { COMMANDS } from "@/lib/constants.generated";
import AssistantModelPicker from "../AssistantModelPicker";

// cmdk calls scrollIntoView, which jsdom does not implement.
Element.prototype.scrollIntoView = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// No `AdvancedSettingsContext` mock here on purpose. This component no longer
// reads that flag at all: it is mounted only inside a `SettingsGroup advanced`,
// so it lists everything (LAC-8407). Mocking the flag is what let this suite
// assert on a state the app cannot produce. The flag's real effect on this
// picker — whether it is mounted — is tested through the real provider in
// AssistantModelPicker.composition.test.tsx.

type Model = {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
  is_legacy: boolean;
};

const CAPABLE: Model = {
  id: "claude-fable-5-1",
  name: "Claude Fable 5.1",
  supports_computer_use: true,
  is_recommended: true,
  is_legacy: false,
};

const LEGACY: Model = {
  id: "claude-haiku-4-5-20251001",
  name: "Claude Haiku 4.5",
  supports_computer_use: true,
  is_recommended: false,
  is_legacy: true,
};

const CHAT_ONLY: Model = {
  id: "some-chat-model",
  name: "Some Chat Model",
  supports_computer_use: false,
  is_recommended: false,
  is_legacy: false,
};

/** Drives the computer through `computer_toolset_20260801`. Juno now sends
 * that toolset, so this is an ordinary capable model with no special mark. */
const TOOLSET: Model = {
  id: "claude-opus-5-5",
  name: "Claude Opus 5.5",
  supports_computer_use: true,
  is_recommended: false,
  is_legacy: false,
};

const settingsWith = (models: Model[], selectedId = models[0].id) =>
  ({
    activeProvider: "anthropic",
    providers: [
      {
        id: "anthropic",
        name: "Anthropic Claude",
        description: "",
        models: models.map((m) => m.id),
        default_model: models[0].id,
        model_info: models,
        is_available: true,
        is_default: true,
        computer_use_supported: true,
      },
    ],
    formData: { model: selectedId },
    providerSettings: { model: selectedId },
    isLoading: false,
    setFormData: vi.fn(),
    handleActiveProviderChange: vi.fn(),
  }) as never;

describe("AssistantModelPicker computer-use marking", () => {
  it("does not mark a model that can control the computer", () => {
    render(<AssistantModelPicker settings={settingsWith([CAPABLE])} />);

    // Absence is the signal: computer use is the norm, so a capable model
    // carries no capability mark at all.
    expect(screen.queryByText("Chat only")).not.toBeInTheDocument();
    expect(screen.queryByText("Computer Use")).not.toBeInTheDocument();
  });

  it("explains the limit when the selected model is chat only", () => {
    render(<AssistantModelPicker settings={settingsWith([CHAT_ONLY])} />);

    expect(screen.getByText(/cannot\s+control the computer/i)).toBeInTheDocument();
  });

  it("treats a toolset model as an ordinary capable model", () => {
    render(<AssistantModelPicker settings={settingsWith([TOOLSET])} />);

    // Juno sends the toolset now, so there is no "Juno cannot drive this"
    // caveat left to show, and it must not be called a chat model either.
    expect(screen.queryByText(/newer tool format/i)).not.toBeInTheDocument();
    expect(screen.queryByText("Chat only")).not.toBeInTheDocument();
    expect(
      screen.queryByText(/cannot\s+control the computer/i)
    ).not.toBeInTheDocument();
  });

  it("says nothing about the limit when the selected model is capable", () => {
    render(<AssistantModelPicker settings={settingsWith([CAPABLE])} />);

    expect(
      screen.queryByText(/cannot\s+control the computer/i)
    ).not.toBeInTheDocument();
  });
});

/** Open the model dropdown and return its list element. The trigger button is
 * labelled with the selected model's name, so select it by position. */
const openModelList = () => {
  fireEvent.click(screen.getAllByRole("button")[0]);
  return screen.getByRole("listbox");
};

describe("AssistantModelPicker model list", () => {
  it("lists every model the provider reports, legacy included", () => {
    // This picker filters nothing. It is advanced-only, and someone who turned
    // advanced settings on is asking to see everything (LAC-8407).
    render(
      <AssistantModelPicker settings={settingsWith([CAPABLE, LEGACY])} />
    );

    const list = openModelList();
    expect(within(list).getByText(CAPABLE.name)).toBeInTheDocument();
    expect(within(list).getByText(LEGACY.name)).toBeInTheDocument();
    expect(within(list).getAllByRole("option")).toHaveLength(2);
  });

  it("lists the model in use even when it is a legacy one", () => {
    // Never hide what Juno is actually running: the person has to be able to
    // see it and switch off it.
    //
    // Not redundant with the test above, and the difference is what it
    // discriminates. The filter LAC-8407 removed carried an exception for the
    // selected model. Re-add that filter WITHOUT the exception and both tests
    // red; re-add it WITH the exception and only the test above reds, because
    // this one's legacy model is the selected one. Keeping both says which
    // variant came back.
    render(
      <AssistantModelPicker
        settings={settingsWith([CAPABLE, LEGACY], LEGACY.id)}
      />
    );

    const list = openModelList();
    expect(within(list).getByText(LEGACY.name)).toBeInTheDocument();
    expect(within(list).getAllByRole("option")).toHaveLength(2);
  });
});

describe("AssistantModelPicker model persistence", () => {
  const pick = (settings: ReturnType<typeof settingsWith>) => {
    render(<AssistantModelPicker settings={settings} />);
    fireEvent.click(
      within(openModelList()).getByText(TOOLSET.name)
    );
  };

  it("does not show a model that failed to save", async () => {
    const settings = settingsWith([CAPABLE, TOOLSET], CAPABLE.id);
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === COMMANDS.PROVIDERS_VALIDATE_PROVIDER_MODEL) return true;
      throw new Error("persist failed");
    });
    const err = vi.spyOn(console, "error").mockImplementation(() => {});

    pick(settings);

    await waitFor(() =>
      expect(err).toHaveBeenCalledWith(
        "AssistantModelPicker: Failed to change model:",
        expect.any(Error)
      )
    );
    expect(
      (settings as unknown as { setFormData: ReturnType<typeof vi.fn> })
        .setFormData
    ).not.toHaveBeenCalled();
    err.mockRestore();
  });

  it("shows the model once it saved", async () => {
    const settings = settingsWith([CAPABLE, TOOLSET], CAPABLE.id);
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(true as never);

    pick(settings);

    const setFormData = (
      settings as unknown as { setFormData: ReturnType<typeof vi.fn> }
    ).setFormData;
    await waitFor(() => expect(setFormData).toHaveBeenCalledTimes(1));
  });
});
