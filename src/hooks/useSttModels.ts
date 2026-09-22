import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

/**
 * Speech-to-text models, as the backend reports them. One status call returns
 * every model's on-disk state and which one the engine is really running; the
 * four actions take a catalog id and the backend dispatches by engine, so this
 * hook (and the UI over it) never knows or cares whether a row is Whisper or
 * Parakeet beyond the name it prints.
 */
export interface SttModelInfo {
	id: string;
	engine: "whisper" | "parakeet";
	name: string;
	size_mb: number;
	downloaded: boolean;
	bundled: boolean;
	active: boolean;
	tier: "fast" | "balanced" | "accurate" | null;
	tier_name: string | null;
	recommended: boolean;
	speed: number;
	accuracy: number;
}

export interface SttDownloadProgress {
	model_id: string;
	bytes_downloaded: number;
	total_bytes: number;
	percent: number;
	activate_when_done: boolean;
}

export interface SttModelsStatus {
	arch: string;
	models: SttModelInfo[];
	active_id: string;
	download: SttDownloadProgress | null;
	offer_recommended: boolean;
}

export interface SttDownloadError {
	modelId: string;
	error: string;
	cancelled: boolean;
}

/** A model that just became active without a tap on Use (a download landed). */
export interface SttModelChanged {
	model_id: string;
	automatic: boolean;
}

function readOnline(): boolean {
	try {
		return typeof navigator === "undefined" ? true : navigator.onLine !== false;
	} catch {
		return true;
	}
}

export function useSttModels() {
	const [status, setStatus] = useState<SttModelsStatus | null>(null);
	const [loading, setLoading] = useState(true);
	const [progress, setProgress] = useState<SttDownloadProgress | null>(null);
	const [downloadError, setDownloadError] = useState<SttDownloadError | null>(null);
	const [actionError, setActionError] = useState<{ modelId: string; error: string } | null>(null);
	const [lastChange, setLastChange] = useState<SttModelChanged | null>(null);
	const [online, setOnline] = useState(readOnline);

	const reload = useCallback(async () => {
		try {
			const next = await invoke<SttModelsStatus>(COMMANDS.STT_MODELS_GET_STATUS);
			setStatus(next);
			setProgress(next.download);
		} catch (error) {
			console.error("Failed to load speech-to-text models:", error);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		reload();
	}, [reload]);

	useEffect(() => {
		const goOnline = () => setOnline(true);
		const goOffline = () => setOnline(false);
		window.addEventListener("online", goOnline);
		window.addEventListener("offline", goOffline);
		return () => {
			window.removeEventListener("online", goOnline);
			window.removeEventListener("offline", goOffline);
		};
	}, []);

	useEventListener<SttDownloadProgress>(EVENTS.STT_MODELS_DOWNLOAD_PROGRESS, (payload) => {
		setProgress(payload);
		setDownloadError(null);
	});

	useEventListener<{ model_id: string; activated: boolean }>(
		EVENTS.STT_MODELS_DOWNLOAD_COMPLETE,
		() => {
			setProgress(null);
			setDownloadError(null);
			reload();
		},
	);

	useEventListener<{ model_id: string; error: string; cancelled: boolean }>(
		EVENTS.STT_MODELS_DOWNLOAD_ERROR,
		(payload) => {
			setProgress(null);
			setDownloadError({
				modelId: payload.model_id,
				error: payload.error,
				cancelled: payload.cancelled,
			});
			reload();
		},
	);

	useEventListener<SttModelChanged>(EVENTS.STT_MODELS_CHANGED, (payload) => {
		setLastChange(payload);
		reload();
	});

	const download = useCallback(
		async (modelId: string) => {
			setDownloadError(null);
			setActionError(null);
			try {
				await invoke(COMMANDS.STT_MODELS_DOWNLOAD, { modelId, activate: false });
				await reload();
			} catch (error) {
				setDownloadError({ modelId, error: String(error), cancelled: false });
			}
		},
		[reload],
	);

	const cancelDownload = useCallback(async () => {
		try {
			await invoke(COMMANDS.STT_MODELS_CANCEL_DOWNLOAD);
		} catch (error) {
			console.error("Failed to cancel download:", error);
		}
	}, []);

	const use = useCallback(
		async (modelId: string) => {
			setActionError(null);
			try {
				await invoke(COMMANDS.STT_MODELS_USE, { modelId });
				setLastChange(null);
				await reload();
			} catch (error) {
				setActionError({ modelId, error: String(error) });
			}
		},
		[reload],
	);

	const remove = useCallback(
		async (modelId: string) => {
			setActionError(null);
			try {
				await invoke(COMMANDS.STT_MODELS_DELETE, { modelId });
				await reload();
			} catch (error) {
				setActionError({ modelId, error: String(error) });
			}
		},
		[reload],
	);

	const dismissChange = useCallback(() => setLastChange(null), []);

	return {
		status,
		loading,
		progress,
		downloadError,
		actionError,
		lastChange,
		online,
		reload,
		download,
		cancelDownload,
		use,
		remove,
		dismissChange,
	};
}

export type SttModelsController = ReturnType<typeof useSttModels>;
