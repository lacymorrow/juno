import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";

import App from "./App";
import ModularSettingsWindow from "./components/settings/ModularSettingsWindow";
import { Toaster } from "./components/ui/sonner";
import { TooltipProvider } from "./components/ui/tooltip";
import { VoiceProvider } from "./contexts/VoiceContext";
import FloatingPanel from "./FloatingPanel";
import OnboardingWindow from "./OnboardingWindow";
import DesktopCursorOverlay from "./components/DesktopCursorOverlay";
import { BarHost } from "./components/bar/BarHost";

import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <VoiceProvider>
      <TooltipProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<App />} />
          <Route path="/settings" element={<ModularSettingsWindow />} />
          <Route path="/app-bar" element={<BarHost />} />
          <Route path="/floating-bar" element={<BarHost />} />
          <Route path="/voice-bar" element={<BarHost />} />
          <Route path="/dynamic-bar" element={<BarHost />} />
          <Route path="/floating-panel" element={<FloatingPanel />} />
          <Route path="/onboarding" element={<OnboardingWindow />} />
          <Route path="/desktop-cursor-overlay" element={<DesktopCursorOverlay />} />
        </Routes>
      </BrowserRouter>
      </TooltipProvider>
      {/* Toast notifications */}
      <Toaster
        position="top-center"
        expand={true}
        richColors={true}
        closeButton={true}
        duration={3000}
        style={{
          fontSize: "14px",
        }}
      />
    </VoiceProvider>
  </React.StrictMode>,
);
