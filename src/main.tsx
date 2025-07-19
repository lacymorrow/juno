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
import { VoiceAIBar } from "./components/bar/voice-ai-bar";
import { DynamicIslandDemo } from "./components/bar/dynamic-bar";
import { ErrorBoundary } from "./components/ErrorBoundary";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary
      onError={(error, errorInfo) => {
        console.error('Root error boundary caught:', error, errorInfo);
        // Log to backend
        import('@tauri-apps/api/core').then(({ invoke }) => {
          invoke('log_frontend_error', {
            error: error.toString(),
            stack: error.stack,
            componentStack: errorInfo.componentStack
          }).catch(console.error);
        });
      }}
    >
      <VoiceProvider>
        <BrowserRouter>
        <Routes>
          <Route path="/" element={<App />} />
          <Route path="/settings" element={<ModularSettingsWindow />} />
          <Route path="/app-bar" element={<AppBar />} />
          <Route path="/floating-bar" element={<FloatingBar />} />
          <Route path="/voice-bar" element={<VoiceAIBar />} />
          <Route path="/dynamic-bar" element={<DynamicIslandDemo />} />
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
    </ErrorBoundary>
  </React.StrictMode>
);
