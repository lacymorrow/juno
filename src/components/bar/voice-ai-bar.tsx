"use client";

import { VoiceAIBarBase } from "./voice-ai-bar-base";
import type { VoiceAIBarProps } from "../../types/voice-ai";

/**
 * Light theme version of the Voice AI Bar component
 * This is a thin wrapper around VoiceAIBarBase with light theme styles
 */
export function VoiceAIBar(props: VoiceAIBarProps) {
  return (
    <>
      <VoiceAIBarBase {...props} theme="light" />
      <style dangerouslySetInnerHTML={{ __html: `
        /* Light theme glass morphism styles */
        .glass-bar-idle {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-idle:hover {
          background: rgba(255, 255, 255, 0.2);
          border-color: rgba(255, 255, 255, 0.3);
          box-shadow: 0 6px 25px rgba(31, 38, 135, 0.4),
            inset 0 3px 15px rgba(255, 255, 255, 0.15);
        }

        .glass-bar-active {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-input {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-response {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-response-width {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-response-height {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-response-summary {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
        }

        .glass-bar-response-expanding {
          background: rgba(255, 255, 255, 0.15);
          border: 1px solid rgba(255, 255, 255, 0.2);
          box-shadow: 0 4px 20px rgba(31, 38, 135, 0.3),
            inset 0 2px 10px rgba(255, 255, 255, 0.1);
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
          background: rgba(0, 0, 0, 0.8);
          backdrop-filter: blur(40px) saturate(200%);
          border: 1px solid rgba(255, 255, 255, 0.1);
          border-radius: 1.5rem;
          box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5),
            0 0 0 1px rgba(255, 255, 255, 0.1) inset,
            0 2px 20px rgba(0, 0, 0, 0.3) inset;
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
            rgba(255, 255, 255, 0.4),
            rgba(255, 255, 255, 0)
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
          background: rgba(255, 255, 255, 0.1);
          border: 1px solid rgba(255, 255, 255, 0.2);
          cursor: pointer;
          transition: all 0.2s;
          flex-shrink: 0;
          color: white;
        }

        .glass-keyboard-btn:hover {
          background: rgba(255, 255, 255, 0.15);
          border-color: rgba(255, 255, 255, 0.3);
          transform: scale(1.05);
        }

        .glass-keyboard-btn:active {
          transform: scale(0.95);
        }
      `}} />
    </>
  );
}