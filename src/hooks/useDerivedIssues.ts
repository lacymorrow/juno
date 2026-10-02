import { useEffect, useRef, useState } from "react";

/**
 * Inline validation that cannot go stale.
 *
 * An inline field error is a statement about the value that is on screen now,
 * so it has to be derived from the value that is on screen now. This hook asks
 * `check` what is wrong with `value` every time `value` changes and replaces
 * the whole answer with the new one, so a message exists for exactly as long as
 * the condition that produces it. Nothing is remembered from a rejected call,
 * which is the bug this exists to make impossible: a refusal cached in local
 * state outlives its cause, and the screen goes on asserting something that
 * stopped being true.
 *
 * It is also not a toast and must never be made to behave like one. Hiding a
 * still-true message after a few seconds leaves a form that refuses input with
 * no visible reason, which is worse than the stale message. The two surfaces
 * divide cleanly: a toast describes an attempt and should disappear on its own,
 * an inline error describes a field and should persist exactly while it holds.
 *
 * `check` should ask the backend for a derivation rather than reimplement the
 * rule, so the rule lives in one place and every screen that shows it shows the
 * same sentence. Keep it referentially stable or hoist it to module scope.
 *
 * @param value   What is on screen. A new identity means a fresh question.
 * @param check   Derives the current issues. Must not mutate `value`.
 * @param enabled While false the answer is empty and nothing is asked, for a
 *                screen that has not loaded its value yet.
 */
export function useDerivedIssues<T, I>(
  value: T,
  check: (value: T) => Promise<I[]>,
  enabled = true,
): I[] {
  const [issues, setIssues] = useState<I[]>([]);

  // Which question is the current one. An answer that lands late describes an
  // earlier value, so it is dropped rather than shown: out-of-order replies are
  // the one way a derived message could still end up stale.
  const latest = useRef(0);

  const checkRef = useRef(check);
  useEffect(() => {
    checkRef.current = check;
  }, [check]);

  useEffect(() => {
    if (!enabled) {
      setIssues([]);
      return;
    }
    const ticket = ++latest.current;
    let live = true;
    const settle = (next: I[]) => {
      if (!live || ticket !== latest.current) return;
      // A backend that answered with something other than a list has told us
      // nothing we can put beside a field, so the field says nothing.
      setIssues(Array.isArray(next) ? next : []);
    };
    void checkRef.current(value).then(settle, () => {
      // The derivation itself failed. Claiming a condition we could not
      // evaluate would be a guess, so the field says nothing.
      settle([]);
    });
    return () => {
      live = false;
    };
  }, [value, enabled]);

  return issues;
}
