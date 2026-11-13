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
	/**
	 * The name of the model that becomes active if this row is removed, on the
	 * active row only; null everywhere else. The confirmation says it, so
	 * removing the model you dictate on is never a silent downgrade.
	 */
	removal_activates: string | null;
}

export interface SttDownloadProgress {
	model_id: string;
	bytes_downloaded: number;
	total_bytes: number;
	percent: number;
	activate_when_done: boolean;
	/**
	 * The connection dropped and the backend is waiting to try again. It
	 * resumes on its own; the UI says so in one plain line.
	 */
	waiting?: boolean;
}

/**
 * The line shown when a download could not even start. The backend's own
 * reason goes to the console, never to the screen: it can carry a URL or an
 * error chain, and neither helps a person decide what to do next.
 */
export const DOWNLOAD_DID_NOT_START = "The download didn't start. Try again.";

/** Shown while a download waits out a lost connection. */
export const DOWNLOAD_WAITING_FOR_NETWORK = "No internet connection. Juno will try again.";

/**
 * Whether model weights can be fetched from Hugging Face on this network.
 * Rust owns this (`src-tauri/src/model_host.rs`); the page prints it and
 * decides nothing. `blocked` is the work-laptop case: the internet works,
 * Hugging Face does not reach the Mac, and no number of retries changes that.
 */
export type ModelHostStatus = "unknown" | "reachable" | "offline" | "blocked";

export interface SttModelsStatus {
	arch: string;
	models: SttModelInfo[];
	active_id: string;
	download: SttDownloadProgress | null;
	offer_recommended: boolean;
	/**
	 * One plain line about a dictation model that has been below this Mac's
	 * Recommended row for more than a day, or null. The backend owns the
	 * wording and the threshold; print it verbatim and never as a popup.
	 */
	quality_notice: string | null;
	network: ModelHostStatus;
	/**
	 * The one line that goes with `network`, or null. Rust owns the
	 * wording; print it verbatim.
	 */
	network_note: string | null;
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

	// The backend re-probes on a network change and after a failed fetch.
	// Nothing here polls; this only redraws when the answer moves.
	useEventListener<ModelHostStatus>(EVENTS.MODEL_HOST_CHANGED, () => {
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
				console.warn("Speech-to-text model download did not start:", error);
				setDownloadError({ modelId, error: DOWNLOAD_DID_NOT_START, cancelled: false });
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

	/**
	 * Ask the network again. Besides the machine changing network this is
	 * the only thing that re-checks: there is deliberately no timer.
	 */
	const retryNetwork = useCallback(async () => {
		setDownloadError(null);
		try {
			await invoke(COMMANDS.MODEL_HOST_RECHECK);
		} catch (error) {
			console.warn("Could not re-check the model host:", error);
		}
		await reload();
	}, [reload]);

	const dismissChange = useCallback(() => setLastChange(null), []);

	return {
		status,
		loading,
		progress,
		downloadError,
		actionError,
		lastChange,
		online,
		/**
		 * `blocked` means Hugging Face cannot be reached though the internet
		 * works. `offline` is already covered by `online` above, which is why
		 * only the blocked line is printed from `networkNote`.
		 */
		network: status?.network ?? "unknown",
		networkNote: status?.network_note ?? null,
		retryNetwork,
		reload,
		download,
		cancelDownload,
		use,
		remove,
		dismissChange,
	};
}

export type SttModelsController = ReturnType<typeof useSttModels>;
