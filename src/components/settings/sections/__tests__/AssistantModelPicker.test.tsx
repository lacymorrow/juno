import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AssistantModelPicker from "../AssistantModelPicker";

// cmdk calls scrollIntoView, which jsdom does not implement.
Element.prototype.scrollIntoView = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const advanced = vi.hoisted(() => ({ on: false }));
vi.mock("../../AdvancedSettingsContext", () => ({
  useAdvancedSettings: () => ({ advanced: advanced.on }),
}));

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
    advanced.on = false;
    render(<AssistantModelPicker settings={settingsWith([CAPABLE])} />);

    // Absence is the signal: computer use is the norm, so a capable model
    // carries no capability mark at all.
    expect(screen.queryByText("Chat only")).not.toBeInTheDocument();
    expect(screen.queryByText("Computer Use")).not.toBeInTheDocument();
  });

  it("explains the limit when the selected model is chat only", () => {
    advanced.on = false;
    render(<AssistantModelPicker settings={settingsWith([CHAT_ONLY])} />);

    expect(screen.getByText(/cannot\s+control the computer/i)).toBeInTheDocument();
  });

  it("treats a toolset model as an ordinary capable model", () => {
    advanced.on = false;
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
    advanced.on = false;
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

describe("AssistantModelPicker legacy gating", () => {
  it("hides older-tool-version models while advanced settings are off", () => {
    advanced.on = false;
    render(
      <AssistantModelPicker settings={settingsWith([CAPABLE, LEGACY])} />
    );

    const list = openModelList();
    expect(within(list).queryByText(LEGACY.name)).not.toBeInTheDocument();
    // the current-generation model is still listed
    expect(within(list).getByText(CAPABLE.name)).toBeInTheDocument();
  });

  it("shows older models once advanced settings are on", () => {
    advanced.on = true;
    render(
      <AssistantModelPicker settings={settingsWith([CAPABLE, LEGACY])} />
    );

    expect(within(openModelList()).getByText(LEGACY.name)).toBeInTheDocument();
  });

  it("always shows the model in use, even when it is an older one", () => {
    // Never hide what Juno is actually running: the person has to be able to
    // see it and switch off it.
    advanced.on = false;
    render(
      <AssistantModelPicker
        settings={settingsWith([CAPABLE, LEGACY], LEGACY.id)}
      />
    );

    expect(within(openModelList()).getByText(LEGACY.name)).toBeInTheDocument();
  });
});
