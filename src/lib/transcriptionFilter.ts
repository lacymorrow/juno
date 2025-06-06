/**
 * Filters out unwanted sequences from transcription text for display purposes.
 * Removes any text in capital letters between brackets (e.g., [BLANK AUDIO], [SILENCE], [NOISE], etc.)
 * while preserving lowercase or mixed-case text in brackets.
 */
export function filterTranscriptionForDisplay(text: string): string {
  // Remove any text in capital letters between brackets
  const filtered = text.replace(/\[\s*[A-Z][A-Z\s]*\]/g, '');
  
  // Clean up multiple spaces and trim
  return filtered
    .split(/\s+/)
    .filter(word => word.length > 0)
    .join(' ')
    .trim();
}