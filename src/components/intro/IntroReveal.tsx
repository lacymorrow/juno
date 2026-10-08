/**
 * IntroReveal: the smoke Juno appears out of (route `/intro`, a transparent
 * click-through window the backend builds around the bar's spot every
 * launch, and destroys when the sequence is over).
 *
 * Display only. On mount it asks the backend for the plan; the answer is the
 * starting gun, because the backend shows this window and starts its own
 * clock the moment it hands the plan over. From then on this draws for
 * `duration_ms` and stops. It decides nothing: the bar is shown by the
 * backend at `bar_at_ms` whether this ever drew a frame.
 *
 * Nothing here is allowed to fail loudly. No WebGL, no plan, no window: the
 * host stays empty and the bar still appears, as it always did. Reduce
 * Motion is the same empty window on purpose.
 */

import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Renderer, Program, Mesh, Triangle } from "ogl";

import { COMMANDS } from "@/lib/constants.generated";
import {
  FRAG,
  VERT,
  prefersReducedMotion,
  progressAt,
  uniformsFor,
  type IntroPlan,
  type IntroUniforms,
} from "./introModel";

/** ogl wants each uniform as `{ value }`. */
const asOgl = (u: IntroUniforms) =>
  Object.fromEntries(Object.entries(u).map(([k, value]) => [k, { value }]));

export const IntroReveal = () => {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let cancelled = false;
    let rafId = 0;
    let renderer: Renderer | null = null;

    const run = async () => {
      let plan: IntroPlan;
      try {
        plan = await invoke<IntroPlan>(COMMANDS.INTRO_READY);
      } catch {
        return;
      }
      // The backend's clock is running either way; with Reduce Motion the
      // bar simply arrives on time with nothing drawn around it.
      if (cancelled || prefersReducedMotion()) return;

      try {
        renderer = new Renderer({ alpha: true, premultipliedAlpha: true });
      } catch {
        return;
      }
      const gl = renderer.gl;
      if (!gl) return;
      gl.clearColor(0, 0, 0, 0);
      host.appendChild(gl.canvas);

      const dpr = Math.min(2, Math.max(1, window.devicePixelRatio || 1));
      const viewport = { width: window.innerWidth, height: window.innerHeight };
      renderer.setSize(viewport.width * dpr, viewport.height * dpr);
      gl.canvas.style.width = `${viewport.width}px`;
      gl.canvas.style.height = `${viewport.height}px`;

      const program = new Program(gl, {
        vertex: VERT,
        fragment: FRAG,
        uniforms: asOgl(uniformsFor(plan, viewport, dpr)),
      });
      const mesh = new Mesh(gl, { geometry: new Triangle(gl), program });

      const start = performance.now();
      const draw = (now: number) => {
        if (cancelled || !renderer) return;
        const t = progressAt(now, start, plan.duration_ms);
        program.uniforms.uT.value = t;
        program.uniforms.uTime.value = (now - start) / 1000;
        renderer.render({ scene: mesh });
        if (t < 1) rafId = requestAnimationFrame(draw);
      };
      rafId = requestAnimationFrame(draw);
    };
    void run();

    return () => {
      cancelled = true;
      cancelAnimationFrame(rafId);
      if (renderer) {
        const gl = renderer.gl;
        gl.getExtension("WEBGL_lose_context")?.loseContext();
        gl.canvas.remove();
      }
    };
  }, []);

  return (
    <>
      <style>{`html, body, #root { background: transparent !important; margin: 0; overflow: hidden; }`}</style>
      <div
        ref={hostRef}
        aria-hidden
        data-testid="intro-reveal"
        style={{ position: "fixed", inset: 0, pointerEvents: "none", overflow: "hidden" }}
      />
    </>
  );
};
