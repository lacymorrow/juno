import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";

import App from "./App";
import ModularSettingsWindow from "./components/settings/ModularSettingsWindow";
import { Toaster } from "./components/ui/sonner";
import { TooltipProvider } from "./components/ui/tooltip";
import { VoiceProvider } from "./contexts/VoiceContext";
import { SettingsProvider } from "./contexts/SettingsContext";
import FloatingPanel from "./FloatingPanel";
import OnboardingWindow from "./OnboardingWindow";
import { DesktopCursorOverlay } from "./components/DesktopCursorOverlay";
import { SnapWellsOverlay } from "./components/SnapWellsOverlay";
import { ListeningGlow } from "./components/ListeningGlow";
import { BarHost } from "./components/bar/BarHost";

// Diagnostic bench for the floating bar. Lazily loaded so its Tauri stand-in is
// installed only when the (unlinked) /__bar-harness route is opened directly,
// never on a normal launch.
const BarStateHarness = lazy(() => import("./bar-harness/BarStateHarness"));

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
      <SettingsProvider>
        <TooltipProvider>
          <BrowserRouter>
            <Routes>
              <Route path="/" element={<App />} />
              <Route path="/settings" element={<ModularSettingsWindow />} />
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
          </BrowserRouter>
        </TooltipProvider>
        {/* Toast notifications. Every knob lives in the Toaster wrapper so all
            windows show the same flat, collapsed stack. */}
        <Toaster />
      </SettingsProvider>
    </VoiceProvider>
  </React.StrictMode>,
);
