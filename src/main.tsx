import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";

import { Toaster } from "./components/ui/sonner";
import { TooltipProvider } from "./components/ui/tooltip";
import { VoiceProvider } from "./contexts/VoiceContext";
// The two windows on screen at launch load with the entry: the bar, and the
// smoke it appears out of. Nothing else is parsed until a window asks for it.
// Every window used to parse the chat, settings, onboarding and every overlay
// (about 3 MB of script) before drawing anything, the bar included.
import { IntroReveal } from "./components/intro/IntroReveal";
import { BarHost } from "./components/bar/BarHost";

const App = lazy(() => import("./App"));
const ModularSettingsWindow = lazy(() => import("./components/settings/ModularSettingsWindow"));
const SettingsProvider = lazy(() =>
  import("./contexts/SettingsContext").then((m) => ({ default: m.SettingsProvider })),
);
const FloatingPanel = lazy(() => import("./FloatingPanel"));
const OnboardingWindow = lazy(() => import("./OnboardingWindow"));
const DesktopCursorOverlay = lazy(() => import("./components/DesktopCursorOverlay"));
const SnapWellsOverlay = lazy(() => import("./components/SnapWellsOverlay"));
const ListeningGlow = lazy(() => import("./components/ListeningGlow"));

// Diagnostic bench for the floating bar. Lazily loaded so its Tauri stand-in is
// installed only when the (unlinked) /__bar-harness route is opened directly,
// never on a normal launch.
const BarStateHarness = lazy(() => import("./bar-harness/BarStateHarness"));
// Live preview of one bar appearance, framed by the settings window's picker.
// Same stand-in layer as the harness, so it never touches a real window.
const AppearancePreview = lazy(() => import("./bar-harness/AppearancePreview"));

import "./styles/globals.css";

// Prevent scrollbar flash during dynamic island transitions in production.
// In dev, leave overflow visible so browser dev tools and layout inspection work normally.
if (import.meta.env.PROD) {
  document.documentElement.style.overflow = "hidden";
  document.body.style.overflow = "hidden";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <VoiceProvider>
      <TooltipProvider>
        <BrowserRouter>
          {/* Fallback null: every lazy window is built hidden or opens over
              nothing, and its chunk is a local file read away. */}
          <Suspense fallback={null}>
            <Routes>
              {/* Only the windows that read settings mount the provider. Its
                  loader fans out a dozen backend calls on mount; on a bar route
                  or a preview frame that was wasted work, and its failure toast
                  stacked up in whichever window happened to host it. */}
              <Route
                path="/"
                element={
                  <SettingsProvider>
                    <App />
                  </SettingsProvider>
                }
              />
              <Route
                path="/settings"
                element={
                  <SettingsProvider>
                    <ModularSettingsWindow />
                  </SettingsProvider>
                }
              />
              <Route path="/app-bar" element={<BarHost />} />
              <Route path="/floating-bar" element={<BarHost />} />
              <Route path="/voice-bar" element={<BarHost />} />
              <Route path="/dynamic-bar" element={<BarHost />} />
              <Route path="/orb-bar" element={<BarHost />} />
              <Route path="/persona-bar" element={<BarHost />} />
              <Route path="/floating-panel" element={<FloatingPanel />} />
              <Route path="/onboarding" element={<OnboardingWindow />} />
              <Route path="/desktop-cursor-overlay" element={<DesktopCursorOverlay />} />
              <Route path="/snap-wells-overlay" element={<SnapWellsOverlay />} />
              <Route path="/listening-overlay" element={<ListeningGlow />} />
              <Route path="/intro" element={<IntroReveal />} />
              <Route
                path="/__bar-preview"
                element={
                  <Suspense fallback={null}>
                    <AppearancePreview />
                  </Suspense>
                }
              />
              {/* Unlinked diagnostic bench; not reachable through normal UI. */}
              <Route
                path="/__bar-harness"
                element={
                  <Suspense fallback={null}>
                    <BarStateHarness />
                  </Suspense>
                }
              />
            </Routes>
          </Suspense>
        </BrowserRouter>
      </TooltipProvider>
      {/* Toast notifications. Every knob lives in the Toaster wrapper so all
          windows show the same flat, collapsed stack. */}
      <Toaster />
    </VoiceProvider>
  </React.StrictMode>,
);
