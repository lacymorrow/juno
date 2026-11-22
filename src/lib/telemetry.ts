import { getVersion } from "@tauri-apps/api/app";
import { load, type Store } from "@tauri-apps/plugin-store";

/**
 * Aptabase telemetry (LAC-7620).
 *
 * Five events and nothing else: app_open, feature_used(name),
 * purchase_started, purchase_completed, error(class). No content, no keys,
 * no identifiers beyond Aptabase's own rotating session id. One switch,
 * default on, persisted in the Tauri store so every window sees the same
 * answer. Turning it off stops events at once.
 */

/** Aptabase Cloud US app key. Ships in the binary by design; not a secret. */
export const APTABASE_APP_KEY = "A-US-6551635026";
const ENDPOINT = "https://us.aptabase.com/api/v0/event";
const STORE_FILE = "telemetry.json";
const STORE_KEY = "enabled";
const SDK_VERSION = "juno-http@1";

/** Session ids rotate after an hour of inactivity, as Aptabase's SDKs do. */
const SESSION_MS = 60 * 60 * 1000;

export type TelemetryEvent =
  | { name: "app_open" }
  | { name: "feature_used"; props: { name: string } }
  | { name: "purchase_started" }
  | { name: "purchase_completed" }
  | { name: "error"; props: { class: string } };

let storePromise: Promise<Store> | null = null;
let session: { id: string; touchedAt: number } | null = null;

function openStore(): Promise<Store> {
  storePromise ??= load(STORE_FILE, { autoSave: true, defaults: {} });
  return storePromise;
}

/** Default on. Any read failure also counts as on: the user never chose off. */
export async function isTelemetryEnabled(): Promise<boolean> {
  try {
    const store = await openStore();
    const value = await store.get<boolean>(STORE_KEY);
    return value !== false;
  } catch {
    return true;
  }
}

export async function setTelemetryEnabled(enabled: boolean): Promise<void> {
  const store = await openStore();
  await store.set(STORE_KEY, enabled);
  await store.save();
}

function sessionId(now: number): string {
  if (!session || now - session.touchedAt > SESSION_MS) {
    session = { id: crypto.randomUUID(), touchedAt: now };
  }
  session.touchedAt = now;
  return session.id;
}

/**
 * Send one event. Never throws and never blocks the caller: telemetry
 * failing must not change what the app does. The switch is read on every
 * call, so a toggle in any window takes effect on the next event.
 */
export async function track(event: TelemetryEvent): Promise<void> {
  try {
    if (!(await isTelemetryEnabled())) return;
    const now = Date.now();
    const version = await getVersion();
    await fetch(ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json", "App-Key": APTABASE_APP_KEY },
      body: JSON.stringify({
        timestamp: new Date(now).toISOString(),
        sessionId: sessionId(now),
        eventName: event.name,
        systemProps: {
          isDebug: import.meta.env.DEV,
          locale: navigator.language,
          appVersion: version,
          sdkVersion: SDK_VERSION,
        },
        props: "props" in event ? event.props : undefined,
      }),
      keepalive: true,
    });
  } catch {
    // Offline, blocked, or the store is unavailable. Drop the event.
  }
}

export const trackAppOpen = () => track({ name: "app_open" });
export const trackFeature = (name: string) => track({ name: "feature_used", props: { name } });
export const trackPurchaseStarted = () => track({ name: "purchase_started" });
export const trackPurchaseCompleted = () => track({ name: "purchase_completed" });
/** Pass a class name such as "TypeError", never a message or stack. */
export const trackError = (errorClass: string) =>
  track({ name: "error", props: { class: errorClass } });
