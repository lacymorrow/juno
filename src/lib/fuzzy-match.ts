/**
 * Lightweight fuzzy matcher for slash-command autocomplete (LAC-3031).
 *
 * Scoring model (highest wins):
 *   - Exact match, then prefix match, dominate everything else.
 *   - Subsequence matches score by proximity: consecutive characters and
 *     characters at word boundaries (after "-", "_", ":", ".") earn bonuses,
 *     gaps and a late first match earn penalties.
 *
 * Returns matched character indices so the UI can highlight them.
 */

export interface FuzzyMatch {
  score: number;
  /** Indices into `target` of the characters that matched `query`. */
  positions: number[];
}

const SCORE_EXACT = 10_000;
const SCORE_PREFIX = 5_000;
const BONUS_CONSECUTIVE = 60;
const BONUS_WORD_BOUNDARY = 40;
const PENALTY_GAP = 2;
const PENALTY_FIRST_MATCH_OFFSET = 4;

const WORD_SEPARATORS = new Set(["-", "_", ":", ".", " "]);

const isBoundary = (target: string, index: number): boolean =>
  index === 0 || WORD_SEPARATORS.has(target[index - 1]);

/**
 * Match `query` against `target` (both case-insensitive).
 * Returns null when `query` is not a subsequence of `target`.
 * An empty query matches everything with score 0.
 */
export const fuzzyMatch = (query: string, target: string): FuzzyMatch | null => {
  const q = query.toLowerCase();
  const t = target.toLowerCase();

  if (q.length === 0) return { score: 0, positions: [] };
  if (q.length > t.length) return null;

  if (q === t) {
    return {
      score: SCORE_EXACT,
      positions: Array.from({ length: t.length }, (_, i) => i),
    };
  }

  if (t.startsWith(q)) {
    return {
      // Shorter targets rank higher among prefix matches ("/pap" prefers
      // "paperclip" over "paperclip-board-tools").
      score: SCORE_PREFIX - t.length,
      positions: Array.from({ length: q.length }, (_, i) => i),
    };
  }

  // Greedy subsequence walk with proximity scoring.
  const positions: number[] = [];
  let score = 0;
  let ti = 0;

  for (let qi = 0; qi < q.length; qi++) {
    const found = t.indexOf(q[qi], ti);
    if (found === -1) return null;

    if (positions.length === 0) {
      score -= found * PENALTY_FIRST_MATCH_OFFSET;
      if (isBoundary(t, found)) score += BONUS_WORD_BOUNDARY;
    } else {
      const prev = positions[positions.length - 1];
      if (found === prev + 1) {
        score += BONUS_CONSECUTIVE;
      } else {
        score -= (found - prev - 1) * PENALTY_GAP;
        if (isBoundary(t, found)) score += BONUS_WORD_BOUNDARY;
      }
    }

    positions.push(found);
    ti = found + 1;
  }

  // Prefer shorter targets when proximity scores tie.
  score -= t.length;

  return { score, positions };
};

export interface RankedItem<T> {
  item: T;
  match: FuzzyMatch;
}

/** Rank `items` by fuzzy score against `query`, dropping non-matches. */
export const rankFuzzy = <T>(
  query: string,
  items: readonly T[],
  getText: (item: T) => string
): RankedItem<T>[] =>
  items
    .map((item) => ({ item, match: fuzzyMatch(query, getText(item)) }))
    .filter((r): r is RankedItem<T> => r.match !== null)
    .sort((a, b) => b.match.score - a.match.score);
