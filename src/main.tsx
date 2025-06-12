import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import App from "./App";
import { FloatingBar } from "./components/FloatingBar";
import ModularSettingsWindow from "./components/settings/ModularSettingsWindow";
import OnboardingWindow from "./OnboardingWindow";
import { Toaster } from "./components/ui/sonner";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<App />} />
          <Route path="/settings" element={<ModularSettingsWindow />} />
          <Route path="/floating-bar" element={<FloatingBar />} />
          <Route path="/onboarding" element={<OnboardingWindow />} />
        </Routes>
      </BrowserRouter>
      <Toaster />
    </>
  </React.StrictMode>
);
