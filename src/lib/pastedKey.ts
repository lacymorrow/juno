/**
 * Find a provider key inside whatever a person pasted.
 *
 * People who are handed a key get it inside a message: "Here's your key:
 * sk-ant-... don't share it". Asking them to select exactly the key is asking
 * them to know what one looks like. They paste the lot; this finds it.
 */

export interface PastedKey {
  key: string;
  provider: { id: string; name: string };
}

// Order matters: an Anthropic key also starts with "sk-". A key starts on its
// own: without the boundary, "risk-assessment-for-the-project" reads as one.
const START = "(?<![A-Za-z0-9_-])";
const PATTERNS: Array<{ id: string; name: string; pattern: RegExp }> = [
  { id: "anthropic", name: "Anthropic", pattern: new RegExp(`${START}sk-ant-[A-Za-z0-9_-]{20,}`) },
  { id: "openai", name: "OpenAI", pattern: new RegExp(`${START}sk-[A-Za-z0-9_-]{20,}`) },
  { id: "gemini", name: "Google Gemini", pattern: new RegExp(`${START}AIza[A-Za-z0-9_-]{30,}`) },
];

export function findPastedKey(text: string): PastedKey | null {
  for (const { id, name, pattern } of PATTERNS) {
    const match = text.match(pattern);
    if (match) return { key: match[0], provider: { id, name } };
  }
  return null;
}
