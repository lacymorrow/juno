import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import App from "./App";
import ModularSettingsWindow from "./components/settings/ModularSettingsWindow";
import { Toaster } from "./components/ui/sonner";
import { VoiceProvider } from "./contexts/VoiceContext";
import FloatingPanel from "./FloatingPanel";
import OnboardingWindow from "./OnboardingWindow";
import DesktopCursorOverlay from "./components/DesktopCursorOverlay";
import "./styles/globals.css";
import { AppBar } from "./components/AppBar";
import { FloatingBar } from "./components/FloatingBar";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <VoiceProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<App />} />
          <Route path="/settings" element={<ModularSettingsWindow />} />
          <Route path="/app-bar" element={<AppBar />} />
          <Route path="/floating-bar" element={<FloatingBar />} />
          <Route path="/floating-panel" element={<FloatingPanel />} />
          <Route path="/onboarding" element={<OnboardingWindow />} />
          <Route
            path="/desktop-cursor-overlay"
            element={<DesktopCursorOverlay />}
          />
        </Routes>
      </BrowserRouter>
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
  </React.StrictMode>
);
