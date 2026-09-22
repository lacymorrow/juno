import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Check, Download, AlertCircle, ChevronRight } from "lucide-react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import AssistantModelPicker from "./AssistantModelPicker";
import { useCallback, useEffect, useState } from "react";

/**
 * Unified Models pane (Advanced only). Two subsections:
 *   1. Dictation model — plain-language speed/accuracy tiers backed by Whisper,
 *      plus an on-device Parakeet engine (arm64) behind a disclosure.
 *   2. Assistant model — the shared agent/computer-use picker.
 *
 * Design note (Jobs standard, principle 2): every dictation tier runs on-device,
 * so a per-card "On device" chip would repeat the same non-distinguishing label
 * three times. It is stated once as the section footer instead.
 */

interface DictationTier {
  key: string;
  name: string;
  /** Whisper model id in the backend MODEL_DEFS. */
  whisperId: string;
  /** Raw checkpoint name shown as the secondary line. */
  checkpoint: string;
  /** 1-5 relative pips. */
  speed: number;
  accuracy: number;
  recommended?: boolean;
}

const DICTATION_TIERS: DictationTier[] = [
  {
    key: "fast",
    name: "Fast",
    whisperId: "tiny-en",
    checkpoint: "Whisper tiny.en",
    speed: 5,
    accuracy: 2,
  },
  {
    key: "balanced",
    name: "Balanced",
    whisperId: "large-v3-turbo",
    checkpoint: "Whisper large-v3-turbo",
    speed: 4,
    accuracy: 4,
    recommended: true,
  },
  {
    key: "accurate",
    name: "Most accurate",
    whisperId: "large-v3",
    checkpoint: "Whisper large-v3",
    speed: 2,
    accuracy: 5,
  },
];

const PARAKEET_MODEL_URL =
  "https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/tree/main/onnx";

/** Flat five-pip meter, system-blue accent, no gradients. */
function PipMeter({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-14 text-[11px] text-muted-foreground">{label}</span>
      <div className="flex gap-1">
        {[1, 2, 3, 4, 5].map((n) => (
          <span
            key={n}
            className={
              n <= value
                ? "h-1.5 w-4 rounded-[2px] bg-[#007AFF]"
                : "h-1.5 w-4 rounded-[2px] bg-black/10 dark:bg-white/15"
            }
          />
        ))}
      </div>
    </div>
  );
}

export default function ModelsSettings({ settings }: SettingsSectionProps) {
  const {
    whisperModels,
    currentWhisperModel,
    whisperDownloading,
    whisperDownloadProgress,
    whisperDownloadError,
    sttProvider,
    parakeetStatus,
    systemArch,
    loadWhisperModels,
    loadSttSettings,
    handleWhisperModelDownload,
    handleWhisperModelChange,
    handleSttProviderChange,
  } = settings;

  const [pendingActivate, setPendingActivate] = useState<string | null>(null);
  const [showExact, setShowExact] = useState(false);

  const isArm = systemArch === "aarch64" || systemArch === "arm64";

  useEffect(() => {
    loadWhisperModels();
    loadSttSettings();
  }, [loadWhisperModels, loadSttSettings]);

  const activateWhisper = useCallback(
    async (whisperId: string) => {
      await handleWhisperModelChange(whisperId);
      if (sttProvider !== "whisper") {
        await handleSttProviderChange("whisper");
      }
    },
    [handleWhisperModelChange, handleSttProviderChange, sttProvider]
  );

  // Refresh downloaded flags whenever a download finishes.
  useEffect(() => {
    if (whisperDownloading === null && pendingActivate) {
      loadWhisperModels();
    }
  }, [whisperDownloading, pendingActivate, loadWhisperModels]);

  // Auto-activate the tier the user asked to download once it lands.
  useEffect(() => {
    if (!pendingActivate) return;
    const model = whisperModels.find((m) => m.id === pendingActivate);
    if (model?.downloaded) {
      activateWhisper(pendingActivate);
      setPendingActivate(null);
    }
  }, [whisperModels, pendingActivate, activateWhisper]);

  // A failed download stops the pending auto-activate; the card shows Retry.
  useEffect(() => {
    if (whisperDownloadError && whisperDownloadError.modelId === pendingActivate) {
      setPendingActivate(null);
    }
  }, [whisperDownloadError, pendingActivate]);

  const anyDictationDownloaded = whisperModels.some((m) => m.downloaded);

  const startDownload = (whisperId: string) => {
    setPendingActivate(whisperId);
    handleWhisperModelDownload(whisperId);
  };

  const renderTierAction = (tier: DictationTier) => {
    const model = whisperModels.find((m) => m.id === tier.whisperId);
    const downloaded = !!model?.downloaded;
    const isDownloading = whisperDownloading === tier.whisperId;
    const isActive =
      sttProvider === "whisper" && currentWhisperModel === tier.whisperId;
    const errored =
      whisperDownloadError?.modelId === tier.whisperId &&
      !isDownloading &&
      !downloaded;
    const sizeMb = model?.size_mb;

    if (isDownloading) {
      const pct = whisperDownloadProgress?.percent ?? 0;
      const doneMb = whisperDownloadProgress
        ? Math.round(whisperDownloadProgress.bytes_downloaded / 1024 / 1024)
        : 0;
      const totalMb = whisperDownloadProgress?.total_bytes
        ? Math.round(whisperDownloadProgress.total_bytes / 1024 / 1024)
        : sizeMb ?? 0;
      return (
        <div className="w-40 space-y-1">
          {whisperDownloadProgress ? (
            <Progress value={pct} className="h-2" />
          ) : (
            <Progress className="h-2 animate-pulse" />
          )}
          <p className="text-[11px] text-muted-foreground">
            {doneMb}
            {totalMb > 0 ? ` / ${totalMb}` : ""} MB
          </p>
        </div>
      );
    }

    if (errored) {
      return (
        <div className="flex flex-col items-end gap-1">
          <span className="flex items-center gap-1 text-[11px] text-[#e8866a]">
            <AlertCircle className="h-3 w-3" /> Download failed
          </span>
          <Button size="sm" variant="outline" onClick={() => startDownload(tier.whisperId)}>
            Retry
          </Button>
        </div>
      );
    }

    if (!downloaded) {
      return (
        <Button size="sm" onClick={() => startDownload(tier.whisperId)}>
          <Download className="mr-1.5 h-3.5 w-3.5" />
          Download{sizeMb ? ` · ${sizeMb} MB` : ""}
        </Button>
      );
    }

    if (isActive) {
      return (
        <span className="flex items-center gap-1.5 text-[13px] font-medium text-[#007AFF]">
          <Check className="h-4 w-4" /> Active
        </span>
      );
    }

    return (
      <Button size="sm" variant="outline" onClick={() => activateWhisper(tier.whisperId)}>
        Use
      </Button>
    );
  };

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Dictation model"
        footer="Runs entirely on your Mac. Your voice is never sent to a server. Balanced is recommended for most people."
      >
        {!anyDictationDownloaded && (
          <SettingsRow
            below={
              <p className="text-sm text-muted-foreground">
                No dictation model downloaded yet. Pick a tier below to download it
                — it activates automatically when the download finishes.
              </p>
            }
          />
        )}

        <SettingsRow
          below={
            <div className="space-y-2.5">
              {DICTATION_TIERS.map((tier) => (
                <div
                  key={tier.key}
                  className="flex items-center justify-between gap-4 rounded-lg border border-black/10 p-3 dark:border-white/10"
                >
                  <div className="min-w-0 space-y-2">
                    <div className="flex items-center gap-2">
                      <span className="text-[15px] font-semibold">{tier.name}</span>
                      {tier.recommended && (
                        <Badge
                          variant="secondary"
                          className="bg-blue-100 text-[10px] text-blue-800 dark:bg-blue-950 dark:text-blue-200"
                        >
                          Recommended
                        </Badge>
                      )}
                    </div>
                    <p className="text-[12px] text-muted-foreground">{tier.checkpoint}</p>
                    <div className="space-y-1">
                      <PipMeter label="Speed" value={tier.speed} />
                      <PipMeter label="Accuracy" value={tier.accuracy} />
                    </div>
                  </div>
                  <div className="shrink-0">{renderTierAction(tier)}</div>
                </div>
              ))}
            </div>
          }
        />

        <SettingsRow
          below={
            <div>
              <button
                type="button"
                onClick={() => setShowExact((v) => !v)}
                className="flex items-center gap-1 text-[12px] text-muted-foreground hover:text-foreground"
              >
                <ChevronRight
                  className={
                    showExact
                      ? "h-3.5 w-3.5 rotate-90 transition-transform"
                      : "h-3.5 w-3.5 transition-transform"
                  }
                />
                Advanced: choose exact model
              </button>

              {showExact && (
                <div className="mt-2 space-y-1.5">
                  {whisperModels.map((model) => {
                    const isActive =
                      sttProvider === "whisper" && currentWhisperModel === model.id;
                    const isDownloading = whisperDownloading === model.id;
                    return (
                      <div
                        key={model.id}
                        className="flex items-center justify-between gap-3 rounded-md bg-black/[0.03] px-3 py-2 dark:bg-white/[0.04]"
                      >
                        <div className="min-w-0">
                          <p className="truncate text-[13px]">{model.display_name}</p>
                          <p className="text-[11px] text-muted-foreground">
                            {model.filename} · {model.size_mb} MB
                          </p>
                        </div>
                        {isActive ? (
                          <span className="flex items-center gap-1 text-[12px] text-[#007AFF]">
                            <Check className="h-3.5 w-3.5" /> Active
                          </span>
                        ) : isDownloading ? (
                          <span className="text-[12px] text-muted-foreground">
                            Downloading…
                          </span>
                        ) : model.downloaded ? (
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => activateWhisper(model.id)}
                          >
                            Use
                          </Button>
                        ) : (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => handleWhisperModelDownload(model.id)}
                          >
                            <Download className="mr-1.5 h-3.5 w-3.5" />
                            Download
                          </Button>
                        )}
                      </div>
                    );
                  })}

                  {isArm && (
                    <div className="flex items-center justify-between gap-3 rounded-md bg-black/[0.03] px-3 py-2 dark:bg-white/[0.04]">
                      <div className="min-w-0">
                        <p className="truncate text-[13px]">Parakeet CTC (on-device)</p>
                        <p className="text-[11px] text-muted-foreground">
                          Experimental ONNX engine.{" "}
                          {parakeetStatus?.downloaded ? (
                            "Model files present."
                          ) : (
                            <>
                              Requires model files —{" "}
                              <a
                                href={PARAKEET_MODEL_URL}
                                target="_blank"
                                rel="noreferrer"
                                className="text-[#007AFF] hover:underline"
                              >
                                download here
                              </a>
                              .
                            </>
                          )}
                        </p>
                      </div>
                      {sttProvider === "parakeet" ? (
                        <span className="flex items-center gap-1 text-[12px] text-[#007AFF]">
                          <Check className="h-3.5 w-3.5" /> Active
                        </span>
                      ) : (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => handleSttProviderChange("parakeet")}
                        >
                          Use
                        </Button>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          }
        />
      </SettingsGroup>

      <AssistantModelPicker
        settings={settings}
        title="Assistant model"
        footer="The model Juno's agent uses to think and drive your computer."
      />
    </div>
  );
}
