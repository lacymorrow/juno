/**
 * Slash-command autocomplete state for text inputs (LAC-3031).
 *
 * When the input holds a single `/token`, this hook fuzzy-matches it against
 * the user's skills (fetched from the Rust backend) and exposes everything a
 * view needs: ranked suggestions, the highlighted selection, inline ghost
 * text for the top match, and a keydown interceptor implementing
 * Tab/Enter = accept, arrows = navigate, Escape = dismiss.
 *
 * Display-layer only — skill enumeration happens in the backend
 * (`list_available_skills`); this hook just renders and routes keys.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import { rankFuzzy, type RankedItem } from "@/lib/fuzzy-match";

export interface SkillInfo {
  name: string;
  description: string;
  source: "skill" | "command";
}

export type SkillSuggestion = RankedItem<SkillInfo>;

const MAX_SUGGESTIONS = 6;

/** `/token` with no whitespace — the only shape we autocomplete. */
const SLASH_TOKEN_RE = /^\/(\S*)$/;

interface UseSkillAutocompleteOptions {
  value: string;
  /** Called with the completed value (e.g. "/paperclip ") on accept. */
  onAccept: (nextValue: string) => void;
  /** Gate the whole feature off (e.g. while the agent is processing). */
  enabled?: boolean;
}

export interface SkillAutocompleteState {
  /** True when the dropdown should be visible. */
  open: boolean;
  suggestions: SkillSuggestion[];
  selectedIndex: number;
  setSelectedIndex: (index: number) => void;
  /**
   * Grey inline completion for the text already typed — the remainder of the
   * selected suggestion when it starts with the typed token, else "".
   */
  ghostText: string;
  /** Accept a suggestion (defaults to the selected one). */
  accept: (index?: number) => void;
  /**
   * Key interceptor — call first in the input's onKeyDown. Calls
   * preventDefault (and stopPropagation) when it consumes the key, which
   * callers/parents must respect by not submitting.
   */
  handleKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void;
}

export const useSkillAutocomplete = ({
  value,
  onAccept,
  enabled = true,
}: UseSkillAutocompleteOptions): SkillAutocompleteState => {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const fetchStateRef = useRef<"idle" | "loading" | "loaded">("idle");

  const token = useMemo(() => {
    if (!enabled) return null;
    const match = SLASH_TOKEN_RE.exec(value);
    return match ? match[1] : null;
  }, [value, enabled]);

  // Lazy-load the skill list the first time a slash token appears.
  // Latches only on success so a transient invoke failure retries on the next
  // keystroke, and survives StrictMode's double effect pass. No mounted-flag:
  // discarding the result would drop the only fetch under StrictMode, and
  // setState after unmount is a safe no-op in React 18.
  useEffect(() => {
    if (token === null || fetchStateRef.current !== "idle") return;
    fetchStateRef.current = "loading";
    invoke<SkillInfo[]>(COMMANDS.SKILLS_LIST_AVAILABLE_SKILLS)
      .then((result) => {
        if (Array.isArray(result)) {
          fetchStateRef.current = "loaded";
          setSkills(result);
        } else {
          fetchStateRef.current = "idle";
        }
      })
      .catch((e) => {
        console.error("SkillAutocomplete: failed to load skills:", e);
        fetchStateRef.current = "idle";
      });
  }, [token]);

  const suggestions = useMemo(() => {
    if (token === null || skills.length === 0) return [];
    return rankFuzzy(token, skills, (s) => s.name).slice(0, MAX_SUGGESTIONS);
  }, [token, skills]);

  // New keystroke → clear a previous Escape-dismissal and reset selection.
  // (Effect keyed on `value` so navigating suggestions doesn't reset.)
  useEffect(() => {
    setDismissed(false);
    setSelectedIndex(0);
  }, [value]);

  const open = suggestions.length > 0 && !dismissed;

  const clampedIndex = Math.min(selectedIndex, suggestions.length - 1);
  const selected = open ? suggestions[clampedIndex] : undefined;

  const ghostText = useMemo(() => {
    if (!selected || token === null) return "";
    const name = selected.item.name;
    if (!name.toLowerCase().startsWith(token.toLowerCase())) return "";
    return name.slice(token.length);
  }, [selected, token]);

  const accept = useCallback(
    (index?: number) => {
      const target = suggestions[index ?? clampedIndex];
      if (!target) return;
      // Trailing space: the command token is complete, keep typing arguments.
      onAccept(`/${target.item.name} `);
      setDismissed(true);
    },
    [suggestions, clampedIndex, onAccept]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (!open) return;
      // Never steal keys from an in-progress IME composition — Enter/Tab there
      // confirms the composed text, not our suggestion.
      if (e.nativeEvent.isComposing || e.key === "Process") return;
      switch (e.key) {
        case "Tab":
        case "Enter":
          e.preventDefault();
          e.stopPropagation();
          accept();
          break;
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((clampedIndex + 1) % suggestions.length);
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex(
            (clampedIndex - 1 + suggestions.length) % suggestions.length
          );
          break;
        case "Escape":
          e.preventDefault();
          e.stopPropagation();
          setDismissed(true);
          break;
        default:
          break;
      }
    },
    [open, accept, clampedIndex, suggestions.length]
  );

  return {
    open,
    suggestions,
    selectedIndex: clampedIndex,
    setSelectedIndex,
    ghostText,
    accept,
    handleKeyDown,
  };
};
