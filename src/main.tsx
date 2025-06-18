import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Toaster } from "sonner";

import App from "./App";
import { VoiceProvider } from "./contexts/VoiceContext";

import "./styles/globals.css";

const root = createRoot(document.getElementById("root") as HTMLElement);

root.render(
  <StrictMode>
    <VoiceProvider>
      <App />
      <Toaster
        position="top-center"
        toastOptions={{
          duration: 3000,
          style: {
            fontSize: "14px",
          },
        }}
      />
    </VoiceProvider>
  </StrictMode>
);
