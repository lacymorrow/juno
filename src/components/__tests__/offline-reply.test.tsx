/**
 * The reply Rust writes when a query cannot reach a model
 * (`connectivity::unanswered_reply`): a plain line and a "Send again" button
 * that resends exactly what was asked. The query is put in a JSX attribute
 * with `&`, `"`, `<` and `>` as entities; this pins that the renderer reads it
 * back unchanged, so the button never sends something the person did not ask.
 */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn((..._args: unknown[]) => Promise.resolve()));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";

/** Mirrors `connectivity::jsx_attr` in Rust. */
const jsxAttr = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const LINE =
  "I can't reach the internet right now, so I can't answer that. Check your Wi-Fi and ask again.";

describe("the offline reply", () => {
  it.each([
    "what's the weather in Charlotte",
    'reply "on my way" to Sam & say <3',
    "set a {timer} for 5 > 4 minutes",
  ])("shows the line and resends %j exactly", async (query) => {
    const content = `${LINE}\n\n<QueryButton query="${jsxAttr(query)}" label="Send again" />`;
    render(<MixedContentRenderer content={content} isStreaming={false} />);

    expect(screen.getByText(LINE)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Send again/ }));
    await act(async () => {});
    expect(invoke).toHaveBeenLastCalledWith("dispatch_query", { query });
  });
});
