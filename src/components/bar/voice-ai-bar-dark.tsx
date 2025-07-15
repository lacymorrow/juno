"use client";

import { VoiceAIBarBase } from "./voice-ai-bar-base";
import type { VoiceAIBarProps } from "../../types/voice-ai";

/**
 * Dark theme version of the Voice AI Bar component
 * This is a thin wrapper around VoiceAIBarBase with dark theme styles
 */
export function VoiceAIBar(props: VoiceAIBarProps) {
  return (
    <>
      <VoiceAIBarBase {...props} theme="dark" />
      <style dangerouslySetInnerHTML={{ __html: `
        /* Dark theme glass morphism styles */
        .glass-bar-idle {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.3) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.4) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.95) 0%,
              rgba(15, 15, 15, 0.95) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.3);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.8),
            inset 0 1px 0 rgba(255, 255, 255, 0.05);
        }

        .glass-bar-idle:hover {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(50, 50, 50, 0.35) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(70, 70, 70, 0.2) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(40, 40, 40, 0.45) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(35, 35, 35, 0.98) 0%,
              rgba(20, 20, 20, 0.98) 100%
            );
          border-color: rgba(80, 80, 80, 0.4);
          box-shadow: 0 12px 40px rgba(0, 0, 0, 0.9),
            inset 0 1px 0 rgba(255, 255, 255, 0.08);
        }

        .glass-bar-active {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-input {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-response {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-response-width {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-response-height {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-response-summary {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        .glass-bar-response-expanding {
          background: radial-gradient(
              circle at 20% 80%,
              rgba(40, 40, 40, 0.25) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 80% 20%,
              rgba(60, 60, 60, 0.15) 0%,
              transparent 50%
            ),
            radial-gradient(
              circle at 40% 40%,
              rgba(30, 30, 30, 0.35) 0%,
              transparent 50%
            ),
            linear-gradient(
              135deg,
              rgba(25, 25, 25, 0.92) 0%,
              rgba(15, 15, 15, 0.92) 100%
            );
          border: 1px solid rgba(60, 60, 60, 0.25);
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.7),
            inset 0 1px 0 rgba(255, 255, 255, 0.04);
        }

        /* Specific size styles */
        .glass-bar-idle {
          padding: 0.4rem;
          width: 120px;
          height: 40px;
        }

        .glass-bar-active {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: 240px;
          height: 40px;
        }

        .glass-bar-input {
          padding: 0.5rem 0.75rem;
          gap: 0.5rem;
          width: 320px;
          height: 40px;
        }

        .glass-bar-response {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: 360px;
          height: 40px;
        }

        .glass-bar-response-width {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: 440px;
          height: 40px;
        }

        .glass-bar-response-height {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: 360px;
          height: 60px;
        }

        .glass-bar-response-summary {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: 420px;
          height: 40px;
        }

        .glass-bar-response-expanding {
          padding: 0.5rem 0.75rem;
          gap: 0.75rem;
          width: var(--expanded-width);
          height: 60px;
        }

        .glass-bar-response-width,
        .glass-bar-response-height,
        .glass-bar-response,
        .glass-bar-response-expanded {
          background: transparent;
          box-shadow: none;
        }

        .glass-bar-response-expanded {
          position: relative;
          background: rgba(10, 10, 10, 0.95);
          backdrop-filter: blur(40px) saturate(200%);
          border: 1px solid rgba(60, 60, 60, 0.2);
          border-radius: 1.5rem;
          box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8),
            0 0 0 1px rgba(255, 255, 255, 0.05) inset,
            0 2px 20px rgba(0, 0, 0, 0.5) inset;
          transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
          animation: none;
          overflow: visible;
        }

        /* Glow effect for glass elements */
        .glass-bar-idle::after,
        .glass-bar-active::after,
        .glass-bar-input::after,
        .glass-bar-response::after,
        .glass-bar-response-width::after,
        .glass-bar-response-height::after,
        .glass-bar-response-summary::after,
        .glass-bar-response-expanding::after {
          content: "";
          position: absolute;
          inset: -1px;
          border-radius: inherit;
          padding: 1px;
          background: linear-gradient(
            135deg,
            rgba(100, 100, 100, 0.3),
            rgba(50, 50, 50, 0)
          );
          mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
          mask-composite: exclude;
          opacity: 0;
          transition: opacity 0.3s;
          pointer-events: none;
        }

        .glass-bar-idle:hover::after,
        .glass-bar-active:hover::after,
        .glass-bar-input:hover::after {
          opacity: 1;
        }

        .glass-keyboard-btn {
          width: 2rem;
          height: 2rem;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(60, 60, 60, 0.2);
          border: 1px solid rgba(80, 80, 80, 0.3);
          cursor: pointer;
          transition: all 0.2s;
          flex-shrink: 0;
          color: white;
        }

        .glass-keyboard-btn:hover {
          background: rgba(80, 80, 80, 0.3);
          border-color: rgba(100, 100, 100, 0.4);
          transform: scale(1.05);
        }

        .glass-keyboard-btn:active {
          transform: scale(0.95);
        }
      `}} />
    </>
  );
}