import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

import ModelsSettings from "../ModelsSettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import { COMMANDS } from "@/lib/constants.generated";
import type { SttModelInfo, SttModelsStatus } from "@/hooks/useSttModels";

const invokeMock = vi.mocked(invoke);

/** The catalog exactly as the backend returns it on Apple Silicon. */
function arm64Models(): SttModelInfo[] {
  return [
    row("tiny-en", "whisper", "Whisper tiny.en", 75, { downloaded: true, bundled: true, active: true, tier: "fast", tier_name: "Fast" }),
    row("parakeet-ctc", "parakeet", "Parakeet CTC 0.6B", 612, { tier: "balanced", tier_name: "Balanced", recommended: true }),
    row("large-v3", "whisper", "Whisper large-v3", 3100, { tier: "accurate", tier_name: "Most accurate" }),
    row("large-v3-turbo", "whisper", "Whisper large-v3-turbo", 600, {}),
    row("small-en", "whisper", "Whisper small.en", 466, {}),
  ];
}

/** ...and on Intel: no Parakeet row, turbo is Balanced. */
function intelModels(): SttModelInfo[] {
  return [
    row("tiny-en", "whisper", "Whisper tiny.en", 75, { downloaded: true, bundled: true, active: true, tier: "fast", tier_name: "Fast" }),
    row("large-v3-turbo", "whisper", "Whisper large-v3-turbo", 600, { tier: "balanced", tier_name: "Balanced", recommended: true }),
    row("large-v3", "whisper", "Whisper large-v3", 3100, { tier: "accurate", tier_name: "Most accurate" }),
    row("small-en", "whisper", "Whisper small.en", 466, {}),
  ];
}

function row(
  id: string,
  engine: SttModelInfo["engine"],
  name: string,
  size_mb: number,
  overrides: Partial<SttModelInfo>,
): SttModelInfo {
  return {
    id,
    engine,
    name,
    size_mb,
    downloaded: false,
    bundled: false,
    active: false,
    tier: null,
    tier_name: null,
    recommended: false,
    speed: 3,
    accuracy: 3,
    ...overrides,
  };
}

function status(models: SttModelInfo[], extra: Partial<SttModelsStatus> = {}): SttModelsStatus {
  const active = models.find((m) => m.active);
  return {
    arch: models.some((m) => m.engine === "parakeet") ? "aarch64" : "x86_64",
    models,
    active_id: active?.id ?? "tiny-en",
    download: null,
    offer_recommended: false,
    ...extra,
  };
}

function mockBackend(current: () => SttModelsStatus, advanced = false) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.STT_MODELS_GET_STATUS) return current();
    if (command === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED) return advanced;
    return undefined;
  });
}

async function mount() {
  render(
    <AdvancedSettingsProvider>
      <ModelsSettings />
    </AdvancedSettingsProvider>,
  );
  await waitFor(() => expect(screen.getByTestId("model-row-tiny-en")).toBeInTheDocument());
}

function rowIds(): string[] {
  return screen
    .getAllByTestId(/^model-row-/)
    .map((el) => el.getAttribute("data-testid")!.replace("model-row-", ""));
}

// src/test/setup.ts makes navigator.onLine writable (not configurable).
function setOnline(online: boolean) {
  (navigator as unknown as { onLine: boolean }).onLine = online;
}

beforeEach(() => {
  invokeMock.mockReset();
  setOnline(true);
});

describe("Models pane: one list, three tiers, full catalog under Advanced", () => {
  it("shows exactly Fast, Balanced, Most accurate on Apple Silicon with Parakeet as Balanced", async () => {
    mockBackend(() => status(arm64Models()));
    await mount();
    expect(rowIds()).toEqual(["tiny-en", "parakeet-ctc", "large-v3"]);
    const balanced = screen.getByTestId("model-row-parakeet-ctc");
    expect(within(balanced).getByText("Balanced")).toBeInTheDocument();
    expect(within(balanced).getByText("Recommended")).toBeInTheDocument();
    expect(within(balanced).getByText(/Parakeet CTC 0.6B · Parakeet · 612 MB/)).toBeInTheDocument();
  });

  it("never renders a Parakeet row on Intel and recommends large-v3-turbo", async () => {
    mockBackend(() => status(intelModels()));
    await mount();
    expect(rowIds()).toEqual(["tiny-en", "large-v3-turbo", "large-v3"]);
    expect(screen.queryByText(/Parakeet/)).not.toBeInTheDocument();
    const balanced = screen.getByTestId("model-row-large-v3-turbo");
    expect(within(balanced).getByText("Recommended")).toBeInTheDocument();
  });

  it("reveals the rest of the catalog as more rows in the same list when Advanced is on", async () => {
    mockBackend(() => status(arm64Models()), true);
    await mount();
    await waitFor(() => expect(rowIds()).toHaveLength(5));
    expect(rowIds()).toEqual(["tiny-en", "parakeet-ctc", "large-v3", "large-v3-turbo", "small-en"]);
    // Still one Active mark, no second section.
    expect(screen.getAllByText("Active")).toHaveLength(1);
  });

  it("says once, for the whole section, that models run on the Mac", async () => {
    mockBackend(() => status(arm64Models()));
    await mount();
    expect(screen.getAllByText(/run on your Mac/)).toHaveLength(1);
  });
});

describe("Models pane: exactly one action per row state, exactly one Active", () => {
  it("renders Download for a model that is not on disk, and never a Use button", async () => {
    mockBackend(() => status(arm64Models()));
    await mount();
    const parakeet = screen.getByTestId("model-row-parakeet-ctc");
    expect(within(parakeet).getByRole("button", { name: /Download: Parakeet/ })).toBeEnabled();
    expect(within(parakeet).queryByRole("button", { name: /^Use/ })).not.toBeInTheDocument();
    expect(within(parakeet).queryByRole("button", { name: /Remove/ })).not.toBeInTheDocument();
  });

  it("renders a progress bar with a cancel while downloading, and disables other Downloads", async () => {
    const models = arm64Models();
    mockBackend(() =>
      status(models, {
        download: {
          model_id: "parakeet-ctc",
          bytes_downloaded: 306 * 1024 * 1024,
          total_bytes: 612 * 1024 * 1024,
          percent: 50,
          activate_when_done: false,
        },
      }),
    );
    await mount();
    const parakeet = screen.getByTestId("model-row-parakeet-ctc");
    expect(within(parakeet).getByLabelText("Download progress")).toBeInTheDocument();
    expect(within(parakeet).getByText(/50% · 306 of 612 MB/)).toBeInTheDocument();
    expect(within(parakeet).getByRole("button", { name: "Cancel download" })).toBeEnabled();
    expect(within(parakeet).queryByRole("button", { name: /Download:/ })).not.toBeInTheDocument();

    const large = screen.getByTestId("model-row-large-v3");
    expect(within(large).getByRole("button", { name: /Download: Whisper large-v3/ })).toBeDisabled();
  });

  it("renders Use (and a trash) for a downloaded, inactive model", async () => {
    const models = arm64Models();
    models[1].downloaded = true;
    mockBackend(() => status(models));
    await mount();
    const parakeet = screen.getByTestId("model-row-parakeet-ctc");
    expect(within(parakeet).getByRole("button", { name: "Use Parakeet CTC 0.6B" })).toBeEnabled();
    expect(within(parakeet).getByRole("button", { name: "Remove Parakeet CTC 0.6B" })).toBeEnabled();
    expect(within(parakeet).queryByRole("button", { name: /Download:/ })).not.toBeInTheDocument();
  });

  it("marks exactly one row Active, as a check and not a button", async () => {
    const models = arm64Models();
    models[0].active = false;
    models[1].downloaded = true;
    models[1].active = true;
    mockBackend(() => status(models, { active_id: "parakeet-ctc" }));
    await mount();
    expect(screen.getAllByText("Active")).toHaveLength(1);
    expect(screen.getByTestId("active-parakeet-ctc")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Active" })).not.toBeInTheDocument();
    // tiny.en is now downloaded-and-inactive: it gets Use, but being bundled, no trash.
    const tiny = screen.getByTestId("model-row-tiny-en");
    expect(within(tiny).getByRole("button", { name: "Use Whisper tiny.en" })).toBeEnabled();
    expect(within(tiny).queryByRole("button", { name: /Remove/ })).not.toBeInTheDocument();
  });

  it("uses a model through the one engine-agnostic command", async () => {
    const models = arm64Models();
    models[1].downloaded = true;
    mockBackend(() => status(models));
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Use Parakeet CTC 0.6B" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.STT_MODELS_USE, { modelId: "parakeet-ctc" }),
    );
  });

  it("shows Try again after a failed download, never a dialog", async () => {
    mockBackend(() => status(arm64Models()));
    invokeMock.mockImplementation(async (command: string) => {
      if (command === COMMANDS.STT_MODELS_GET_STATUS) return status(arm64Models());
      if (command === COMMANDS.STT_MODELS_DOWNLOAD) throw new Error("Could not reach huggingface.co");
      return false;
    });
    await mount();
    fireEvent.click(screen.getByRole("button", { name: /Download: Parakeet/ }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Try again: Parakeet/ })).toBeEnabled(),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(/Download failed/);
  });

  it("disables Download with a reason when offline; downloaded models are unaffected", async () => {
    setOnline(false);
    const models = arm64Models();
    models[1].downloaded = true;
    mockBackend(() => status(models));
    await mount();
    expect(screen.getByRole("button", { name: /Download: Whisper large-v3/ })).toBeDisabled();
    expect(screen.getByText(/No internet connection/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use Parakeet CTC 0.6B" })).toBeEnabled();
  });
});

describe("Models pane: delete", () => {
  it("never offers the trash for the bundled model or the active model", async () => {
    const models = arm64Models();
    models[1].downloaded = true; // parakeet: downloaded, inactive -> deletable
    mockBackend(() => status(models));
    await mount();
    expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Remove Parakeet CTC 0.6B" })).toBeInTheDocument();
  });

  it("confirms inline with one tap and then calls the one delete command", async () => {
    const models = arm64Models();
    models[1].downloaded = true;
    mockBackend(() => status(models));
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Remove Parakeet CTC 0.6B" }));
    const confirm = screen.getByRole("group", { name: "Remove Parakeet CTC 0.6B?" });
    expect(within(confirm).getByText("Remove?")).toBeInTheDocument();
    fireEvent.click(within(confirm).getByRole("button", { name: "Confirm remove Parakeet CTC 0.6B" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.STT_MODELS_DELETE, { modelId: "parakeet-ctc" }),
    );
  });

  it("Keep backs out without calling delete", async () => {
    const models = arm64Models();
    models[1].downloaded = true;
    mockBackend(() => status(models));
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Remove Parakeet CTC 0.6B" }));
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(screen.getByRole("button", { name: "Use Parakeet CTC 0.6B" })).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith(COMMANDS.STT_MODELS_DELETE, expect.anything());
  });
});
