import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { COMMANDS, SETTINGS } from "@/lib/constants.generated";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import AIProviderSettings from "../AIProviderSettings";

/**
 * The test LAC-8407 was missing, and the reason the defect survived three PRs.
 *
 * Every other test of this picker mocks `useAdvancedSettings`. That mock is
 * what hid the bug: it lets the suite set `advanced: false` *while the picker
 * is mounted*, and the shipping app cannot produce that state, because the
 * `SettingsGroup advanced` wrapping the picker is gated on the very same flag.
 * So the suite asserted on a filter that could never run, and passed.
 *
 * These tests mount the real `AdvancedSettingsProvider` around the picker's
 * only call site, so one flag reaches both the group and the picker the way it
 * does in the app. That composition is the thing under test; mocking it away is
 * what made a dead branch look alive.
 */

// cmdk calls scrollIntoView, which jsdom does not implement.
Element.prototype.scrollIntoView = vi.fn();

// The mock factory is hoisted above the imports, so it cannot read COMMANDS.
// These literals are pinned against the generated constants by the first test
// below, so a renamed command fails loudly instead of silently never matching
// (a `default:` that resolves is indistinguishable from a command that is not
// called at all).
const backend = vi.hoisted(() => ({
  advancedEnabled: false,
  GET_ADVANCED: "get_advanced_settings_enabled",
  GET_DEMO: "get_demo_info",
  CHECK_CLI: "check_claude_cli_available",
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string) => {
    switch (command) {
      case backend.GET_ADVANCED:
        return Promise.resolve(backend.advancedEnabled);
      case backend.GET_DEMO:
        // Demo access is the one group in this section that is NOT gated on the
        // advanced flag, so it is the anchor that proves the section rendered
        // when the assertion below is that the picker did not.
        return Promise.resolve({ is_demo: true, cohort: null });
      case backend.CHECK_CLI:
        return Promise.resolve({ available: false, authenticated: false });
      default:
        return Promise.resolve(undefined);
    }
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

type Model = {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
  is_legacy: boolean;
};

/** Current and recommended — the shipping default. */
const CURRENT: Model = {
  id: "claude-opus-5-5",
  name: "Claude Opus 5.5",
  supports_computer_use: true,
  is_recommended: true,
  is_legacy: false,
};

/** A second current model, so the list has something to switch *to*. */
const ALSO_CURRENT: Model = {
  id: "claude-haiku-5-5",
  name: "Claude Haiku 5.5",
  supports_computer_use: true,
  is_recommended: false,
  is_legacy: false,
};

/** Legacy, so it is the model the filter LAC-8407 removed would have hidden.
 *  If that filtering comes back, this is the one that disappears.
 *
 *  That filter had a second axis, a capability flag for models that drive the
 *  computer only through `computer_toolset_20260801`. It is gone: LAC-7431
 *  deleted the field from the wire format, so this fixture cannot express it
 *  and nothing can filter on it. The deletion is what replaces that half of
 *  the guard — re-adding the capability filter is now a type error rather than
 *  a test failure, which is the stronger check. `is_legacy` keeps a real
 *  fixture because it may yet gain a reader (LAC-8418). */
const LEGACY: Model = {
  id: "claude-haiku-4-5-20251001",
  name: "Claude Haiku 4.5",
  supports_computer_use: true,
  is_recommended: false,
  is_legacy: true,
};

const ALL_MODELS = [CURRENT, ALSO_CURRENT, LEGACY];

const settings = (models: Model[] = ALL_MODELS, selectedId = CURRENT.id) =>
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
        needs_sign_in: false,
        has_saved_key: false,
      },
    ],
    formData: {
      model: selectedId,
      apiKey: "",
      systemPrompt: "",
      maxTokens: 1024,
      temperature: 1,
    },
    providerSettings: { model: selectedId, load_account_mcp: true },
    isLoading: false,
    setFormData: vi.fn(),
    handleActiveProviderChange: vi.fn(),
    handleSaveProviderSettings: vi.fn(),
    handleClearProviderApiKey: vi.fn(),
    handleLoadAccountMcpChange: vi.fn(),
  }) as never;

const renderSection = (models: Model[] = ALL_MODELS) =>
  render(
    <AdvancedSettingsProvider>
      <AIProviderSettings settings={settings(models)} />
    </AdvancedSettingsProvider>
  );

/** The model trigger is labelled with the selected model's name. */
const openModelList = () => {
  fireEvent.click(screen.getByRole("button", { name: /Claude Opus 5\.5/ }));
  return screen.getByRole("listbox");
};

describe("AssistantModelPicker, mounted the way the app mounts it", () => {
  beforeEach(() => {
    backend.advancedEnabled = false;
  });

  it("pins the command names this file's hoisted mock matches on", () => {
    // Guards the mock itself: a rename would make every case fall through to
    // `default`, and the tests below would then be asserting against an
    // advanced flag that never loaded rather than against the real thing.
    expect(backend.GET_ADVANCED).toBe(
      COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED
    );
    expect(backend.GET_DEMO).toBe(COMMANDS.CORE_GET_DEMO_INFO);
    expect(backend.CHECK_CLI).toBe(
      COMMANDS.ONBOARDING_CHECK_CLAUDE_CLI_AVAILABLE
    );
    // The provider also follows this event; named here so a rename is caught
    // in one place with the commands.
    expect(SETTINGS.EVENTS_ADVANCED_SETTINGS_CHANGED).toBe(
      "advanced_settings_changed"
    );
  });

  it("renders no model picker at all while advanced settings are off", async () => {
    // This is the fact that made the legacy filter dead code: with the toggle
    // off there is no picker to filter. QA observed the same thing on a real
    // build (LAC-7423): "advanced OFF: there is no model picker at all".
    renderSection();

    // Anchor first, so the absence assertions below cannot pass vacuously
    // against a section that simply had not rendered yet.
    expect(await screen.findByText("Demo access")).toBeInTheDocument();

    expect(screen.queryByText("Provider Selection")).not.toBeInTheDocument();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    for (const model of ALL_MODELS) {
      expect(screen.queryByText(model.name)).not.toBeInTheDocument();
    }
  });

  it("lists every model the provider reports, legacy included, in the only state where it exists", async () => {
    // The negative control for LAC-8407's option (b). Any filtering re-added to
    // this picker drops LEGACY (it is both `is_legacy` and off the toolset) and
    // reds the count, which is the behaviour change a reader can actually see.
    // Note what this deliberately does NOT red on: re-adding a filter keyed on
    // `showAdvanced`, because that filter changes nothing a person sees here —
    // which is the whole finding, and why the guard against it is the comment
    // in the component, not an assertion.
    backend.advancedEnabled = true;
    renderSection();

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /Claude Opus 5\.5/ })
      ).toBeInTheDocument()
    );

    const list = openModelList();
    for (const model of ALL_MODELS) {
      expect(within(list).getByText(model.name)).toBeInTheDocument();
    }
    expect(within(list).getAllByRole("option")).toHaveLength(ALL_MODELS.length);
  });

  it("keeps a provider whose model list is empty out of the dropdown", async () => {
    // The one piece of `sortedProviders` filtering that survived: it now turns
    // on `model_info.length`, not on per-model visibility.
    backend.advancedEnabled = true;
    render(
      <AdvancedSettingsProvider>
        <AIProviderSettings
          settings={
            {
              ...(settings() as unknown as Record<string, unknown>),
              providers: [
                (settings() as unknown as { providers: unknown[] }).providers[0],
                {
                  id: "openai",
                  name: "OpenAI",
                  description: "",
                  models: [],
                  default_model: "",
                  model_info: [],
                  is_available: true,
                  is_default: false,
                  computer_use_supported: true,
                },
              ],
            } as never
          }
        />
      </AdvancedSettingsProvider>
    );

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /Claude Opus 5\.5/ })
      ).toBeInTheDocument()
    );

    const list = openModelList();
    expect(within(list).queryByText("OpenAI")).not.toBeInTheDocument();
    expect(within(list).getAllByRole("option")).toHaveLength(ALL_MODELS.length);
  });
});
