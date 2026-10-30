import { useEffect, useRef } from "react";

/**
 * Type to focus, the way Slack, Discord and Linear do it: with the window key
 * and nothing editable focused, a printable key (or Cmd+V) puts the caret in
 * the composer, and the character lands in it.
 *
 * Keys only arrive when Juno's window is key, so this adds no global hotkey
 * and never reaches outside the window.
 */

/** The parts of a KeyboardEvent the decision reads, so the predicate tests without a DOM. */
export interface TypeToFocusKey {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  isComposing?: boolean;
  keyCode?: number;
  defaultPrevented?: boolean;
}

export interface TypeToFocusContext {
  /** The event target is an input, textarea, select or contenteditable. */
  targetEditable: boolean;
  /** A menu, dialog or popover is open and owns the keyboard. */
  overlayOpen: boolean;
  /** A dictation or voice turn is recording: keys are not for the box. */
  recording: boolean;
}

/** One visible character: not Space (it presses a focused button), not a named key. */
function isPrintable(key: string): boolean {
  return key !== " " && [...key].length === 1;
}

/** Cmd+V, with Shift allowed (paste without formatting). Alt and Ctrl are other chords. */
function isPaste(e: TypeToFocusKey): boolean {
  return e.metaKey && !e.ctrlKey && !e.altKey && e.key.toLowerCase() === "v";
}

/** Should this keydown move the caret into the composer? Pure. */
export function wantsComposer(e: TypeToFocusKey, ctx: TypeToFocusContext): boolean {
  if (e.defaultPrevented || ctx.targetEditable || ctx.overlayOpen || ctx.recording) return false;
  // An IME in the middle of a composition owns the key (keyCode 229 on the
  // keydown that starts one).
  if (e.isComposing || e.keyCode === 229) return false;
  if (isPaste(e)) return true;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  return isPrintable(e.key);
}

/** An element the person can type into. */
export function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || typeof el.tagName !== "string") return false;
  const tag = el.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  if (el.isContentEditable) return true;
  const editable = el.closest?.("[contenteditable]")?.getAttribute("contenteditable");
  return editable !== null && editable !== undefined && editable !== "false";
}

const OVERLAY_SELECTOR = '[role="menu"], [role="dialog"], [role="alertdialog"], [role="listbox"]';

/** A Radix menu, dialog, popover or listbox is on screen (tooltips are not). */
export function overlayIsOpen(doc: Document = document): boolean {
  return doc.querySelector(OVERLAY_SELECTOR) !== null;
}

/**
 * Wire it: `focus` puts the caret in the composer; `recording` turns it off
 * while a microphone is open. The keydown is not prevented, so once the
 * composer has focus the browser inserts the character into it.
 */
export function useTypeToFocus({ focus, recording }: { focus: () => void; recording: boolean }): void {
  const focusRef = useRef(focus);
  focusRef.current = focus;
  const recordingRef = useRef(recording);
  recordingRef.current = recording;
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ctx: TypeToFocusContext = {
        targetEditable: isEditableTarget(e.target),
        overlayOpen: overlayIsOpen(),
        recording: recordingRef.current,
      };
      if (wantsComposer(e, ctx)) focusRef.current();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);
}
