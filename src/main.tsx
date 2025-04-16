import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import App from "./App";
import FloatingBar from "./FloatingBar";
import "./styles/globals.css";
// import "./styles/App.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<App />} />
        <Route path="/floating-bar" element={<FloatingBar />} />
      </Routes>
    </BrowserRouter>
  </React.StrictMode>
);
