import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Check, Download, Loader2, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useAdvancedSettings } from "../AdvancedSettingsContext";
import { SettingsGroup, SettingsRow } from "../ui";
import {
  useSttModels,
  type SttDownloadProgress,
  type SttModelInfo,
  type SttModelsController,
} from "@/hooks/useSttModels";

/**
 * Settings > Models: one list of dictation models.
 *
 * Everyone sees three rows (Fast, Balanced, Most accurate). Advanced settings
 * reveal the rest of the catalog as more rows in the same list. Each row has
 * exactly one action for its state: Download, a progress bar with a cancel,
 * Use, or an Active check. A trash icon appears only on downloaded, inactive
 * rows; the bundled model is never deletable. The backend decides all of it:
 * which rows exist for this Mac, which one the engine is really running, and
 * what is on disk. This file only draws that.
 */

const MB = 1024 * 1024;

function formatMb(bytes: number): string {
  return `${Math.round(bytes / MB)}`;
}

/** Flat five-pip meter, system blue, no gradients. */
function PipMeter({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center gap-2" aria-label={`${label} ${value} of 5`}>
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

function engineLabel(engine: SttModelInfo["engine"]): string {
  return engine === "parakeet" ? "Parakeet" : "Whisper";
}

interface RowProps {
  model: SttModelInfo;
  stt: SttModelsController;
}

function DownloadingAction({
  progress,
  onCancel,
}: {
  progress: SttDownloadProgress | null;
  onCancel: () => void;
}) {
  const pct = progress ? Math.min(100, Math.max(0, progress.percent)) : 0;
  const total = progress?.total_bytes ?? 0;
  return (
    <div className="flex w-44 items-center gap-2">
      <div className="min-w-0 flex-1 space-y-1">
        {progress ? (
          <Progress value={pct} className="h-1.5" aria-label="Download progress" />
        ) : (
          <Progress className="h-1.5 animate-pulse" aria-label="Starting download" />
        )}
        <p className="text-[11px] tabular-nums text-muted-foreground">
          {progress
            ? `${Math.round(pct)}% · ${formatMb(progress.bytes_downloaded)}${
                total > 0 ? ` of ${formatMb(total)}` : ""
              } MB`
            : "Starting…"}
        </p>
      </div>
      <Button
        size="icon-xs"
        variant="ghost"
        onClick={onCancel}
        aria-label="Cancel download"
        title="Cancel download"
      >
        <X />
      </Button>
    </div>
  );
}

function ModelRow({ model, stt }: RowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [busy, setBusy] = useState<"use" | "delete" | null>(null);

  const downloading = stt.progress?.model_id === model.id;
  const anotherDownloading = !!stt.progress && !downloading;
  const failed =
    stt.downloadError?.modelId === model.id && !stt.downloadError.cancelled && !model.downloaded;
  const actionError = stt.actionError?.modelId === model.id ? stt.actionError.error : null;

  // Leave the inline confirmation if the row stops being deletable underneath it.
  useEffect(() => {
    if (!model.downloaded || model.active || downloading) setConfirmDelete(false);
  }, [model.downloaded, model.active, downloading]);

  const runUse = async () => {
    setBusy("use");
    try {
      await stt.use(model.id);
    } finally {
      setBusy(null);
    }
  };

  const runDelete = async () => {
    setBusy("delete");
    try {
      await stt.remove(model.id);
    } finally {
      setBusy(null);
      setConfirmDelete(false);
    }
  };

  let action: React.ReactNode;
  if (downloading) {
    action = <DownloadingAction progress={stt.progress} onCancel={stt.cancelDownload} />;
  } else if (model.active) {
    action = (
      <span
        className="flex items-center gap-1.5 text-[13px] font-medium text-[#007AFF]"
        data-testid={`active-${model.id}`}
      >
        <Check className="h-4 w-4" aria-hidden /> Active
      </span>
    );
  } else if (model.downloaded) {
    action = (
      <Button
        size="sm"
        variant="outline"
        onClick={runUse}
        disabled={busy !== null}
        aria-label={`Use ${model.name}`}
      >
        {busy === "use" ? <Loader2 className="animate-spin" aria-hidden /> : null}
        Use
      </Button>
    );
  } else {
    const label = failed ? "Try again" : "Download";
    action = (
      <Button
        size="sm"
        onClick={() => stt.download(model.id)}
        disabled={!stt.online || anotherDownloading}
        aria-label={`${label}: ${model.name}`}
        title={
          !stt.online
            ? "No internet connection"
            : anotherDownloading
              ? "Another download is in progress"
              : undefined
        }
      >
        <Download aria-hidden />
        {label}
      </Button>
    );
  }

  const canDelete = model.downloaded && !model.bundled && !model.active && !downloading;

  return (
    <div
      className="flex items-start justify-between gap-4 px-4 py-3"
      data-testid={`model-row-${model.id}`}
    >
      <div className="min-w-0 space-y-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5">
          <span className="text-[14px] font-semibold">
            {model.tier_name ?? model.name}
          </span>
          {model.recommended && (
            <span className="rounded-full bg-[#007AFF]/10 px-1.5 py-px text-[10px] font-medium text-[#007AFF]">
              Recommended
            </span>
          )}
        </div>
        <p className="text-[12px] text-muted-foreground">
          {model.tier_name ? `${model.name} · ` : ""}
          {engineLabel(model.engine)} · {model.size_mb} MB
          {model.bundled ? " · Included" : ""}
        </p>
        <div className="space-y-1 pt-0.5">
          <PipMeter label="Speed" value={model.speed} />
          <PipMeter label="Accuracy" value={model.accuracy} />
        </div>
        {failed && (
          <p className="text-[11px] text-[#e8866a]" role="alert">
            Download failed. {stt.downloadError?.error}
          </p>
        )}
        {actionError && (
          <p className="text-[11px] text-[#e8866a]" role="alert">
            {actionError}
          </p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
        {confirmDelete ? (
          <div className="flex items-center gap-1.5" role="group" aria-label={`Remove ${model.name}?`}>
            <span className="text-[12px] text-muted-foreground">Remove?</span>
            <Button
              size="sm"
              variant="destructive"
              onClick={runDelete}
              disabled={busy !== null}
              aria-label={`Confirm remove ${model.name}`}
            >
              Remove
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setConfirmDelete(false)}
              disabled={busy !== null}
            >
              Keep
            </Button>
          </div>
        ) : (
          <>
            {action}
            {canDelete && (
              <Button
                size="icon-xs"
                variant="ghost"
                className="text-muted-foreground"
                onClick={() => setConfirmDelete(true)}
                aria-label={`Remove ${model.name}`}
                title="Remove from this Mac"
              >
                <Trash2 />
              </Button>
            )}
          </>
        )}
      </div>
    </div>
  );
}

export default function ModelsSettings() {
  const stt = useSttModels();
  const { advanced } = useAdvancedSettings();

  const models = stt.status?.models ?? [];
  const visible = advanced ? models : models.filter((m) => m.tier !== null);
  const downloadingModel = stt.progress
    ? models.find((m) => m.id === stt.progress?.model_id)
    : undefined;
  const changed = stt.lastChange?.automatic
    ? models.find((m) => m.id === stt.lastChange?.model_id)
    : undefined;

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Dictation model"
        footer="All models run on your Mac. Your voice never leaves it."
      >
        {(downloadingModel || changed || !stt.online) && (
          <SettingsRow
            below={
              <div className="space-y-1 text-[12px] text-muted-foreground" role="status">
                {downloadingModel && (
                  <p className="flex items-center gap-1.5">
                    <Loader2 className="h-3 w-3 animate-spin" aria-hidden />
                    Downloading {downloadingModel.name}
                    {stt.progress?.activate_when_done
                      ? ". Dictation keeps working; it switches over when this lands."
                      : "."}
                  </p>
                )}
                {changed && (
                  <p className="flex items-center justify-between gap-2">
                    <span>{changed.name} is now active.</span>
                    <button
                      type="button"
                      onClick={stt.dismissChange}
                      className="text-[11px] hover:text-foreground"
                    >
                      OK
                    </button>
                  </p>
                )}
                {!stt.online && <p>No internet connection. Downloaded models still work.</p>}
              </div>
            }
          />
        )}

        {stt.loading && models.length === 0 ? (
          <SettingsRow
            below={
              <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" aria-hidden /> Checking models…
              </p>
            }
          />
        ) : (
          visible.map((model) => <ModelRow key={model.id} model={model} stt={stt} />)
        )}
      </SettingsGroup>
    </div>
  );
}
