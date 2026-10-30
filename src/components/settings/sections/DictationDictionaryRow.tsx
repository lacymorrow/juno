import { Input } from "@/components/ui/input";
import { COMMANDS } from "@/lib/constants.generated";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingsRow } from "../ui";

export const DICTIONARY_COPY =
  "Names Juno hears but spells wrong. Add them the way you want them typed.";
export const DICTIONARY_EMPTY = "Nothing yet. Start with a name Juno gets wrong.";
export const DICTIONARY_PLACEHOLDER = "Add a word, press Return";

/**
 * "Words to spell right": the personal dictionary. The list and the matching
 * live in Rust; this draws the words as chips with an input to add one and an
 * x to remove one. Every change is saved at once, and Rust hands back the
 * tidied list (trimmed, no blanks, no repeats), which is what is shown.
 */
export function DictationDictionaryRow() {
  const [words, setWords] = useState<string[] | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let mounted = true;
    invoke<string[]>(COMMANDS.DICTATION_GET_DICTATION_DICTIONARY)
      .then((list) => {
        if (mounted && Array.isArray(list)) setWords(list);
      })
      .catch(() => {
        // No list means no row: better than an editor that saves nowhere.
      });
    return () => {
      mounted = false;
    };
  }, []);

  if (words === null) return null;

  const save = async (next: string[]) => {
    const previous = words;
    setWords(next);
    try {
      const saved = await invoke<string[]>(
        COMMANDS.DICTATION_SET_DICTATION_DICTIONARY,
        { words: next },
      );
      if (Array.isArray(saved)) setWords(saved);
    } catch {
      setWords(previous);
    }
  };

  const add = () => {
    const word = draft.trim();
    if (!word) return;
    setDraft("");
    if (words.some((w) => w.toLowerCase() === word.toLowerCase())) return;
    void save([...words, word]);
  };

  return (
    <SettingsRow
      id="dictation-dictionary"
      htmlFor="dictation-dictionary-input"
      label="Words to spell right"
      description={DICTIONARY_COPY}
      below={
        <div className="flex flex-col gap-2">
          {words.length === 0 ? (
            <p className="text-[12px] text-muted-foreground">{DICTIONARY_EMPTY}</p>
          ) : (
            <ul className="flex flex-wrap gap-1.5" aria-label="Dictionary words">
              {words.map((word) => (
                <li
                  key={word}
                  className="inline-flex h-6 items-center gap-1 rounded-[6px] bg-muted pl-2 pr-1 text-[12px] leading-none"
                >
                  <span>{word}</span>
                  <button
                    type="button"
                    aria-label={`Remove ${word}`}
                    className="inline-flex h-4 w-4 items-center justify-center rounded-[4px] text-muted-foreground hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    onClick={() => void save(words.filter((w) => w !== word))}
                  >
                    <X className="h-3 w-3" aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
          )}
          <Input
            id="dictation-dictionary-input"
            value={draft}
            placeholder={DICTIONARY_PLACEHOLDER}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            className="h-7 max-w-[260px] text-[12px]"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
          />
        </div>
      }
    />
  );
}
