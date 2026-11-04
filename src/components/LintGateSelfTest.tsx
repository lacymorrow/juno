// Deliberate boundary + hooks violations. This file exists to prove CI's lint
// gate rejects them (LAC-3998). It is never merged.
import { useState } from 'react';

export const LintGateSelfTest = () => {
  const [n, setN] = useState(0);

  // Boundary breach: settings persistence is Rust's (Tauri Store).
  localStorage.setItem('lint-gate-selftest', String(n));

  // Hooks breach: state set during render.
  setN(n + 1);

  return <div>{n}</div>;
};
