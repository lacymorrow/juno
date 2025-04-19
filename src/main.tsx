import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import App from "./App";
import { FloatingBar } from "./Bar";
import { Toaster } from "./components/ui/sonner";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<App />} />
          <Route path="/floating-bar" element={<FloatingBar />} />
        </Routes>
      </BrowserRouter>
      <Toaster />
    </>
  </React.StrictMode>
);
