//! The personal dictionary: words a speech model keeps misspelling, and how
//! the person wants them written.
//!
//! A speech model knows the words in its training data. A name it has never
//! seen comes out as a guess at the spelling: "Juno" is typed as "Juneau",
//! "joono" or "Juneou", and no amount of model quality fixes that, because
//! there is nothing to be more accurate about. Superwhisper and Wispr Flow
//! solve it the same way: the person keeps a short list of their own words.
//!
//! This is that list, applied to a transcript. Each heard word is compared to
//! each entry by how it sounds, with the matcher the wake phrase already uses
//! ([`crate::wake_word`]), and a match is replaced by the entry as the person
//! spelled it, capitals included. Everything around the word (spacing,
//! punctuation, the possessive "s" that got split off) is left exactly as it
//! was. An entry may be more than one word ("Build and Serve"); it then has
//! to be heard as adjacent words.
//!
//! What this cannot do, deliberately: a name misheard as a different phrase
//! ("do you know" for "Juno") stays as heard. Those words are real English
//! the person might have meant, and a dictionary that rewrites them would be
//! wrong more often than right.

use crate::wake_word::{phonetic_key, tokenize, words_match};

/// One entry as the person wrote it, with the per-word keys the matcher
/// compares against.
struct Entry<'a> {
    text: &'a str,
    words: Vec<String>,
    keys: Vec<String>,
}

/// A word in the transcript: its byte span and its lowercase form.
struct Heard {
    start: usize,
    end: usize,
    lower: String,
    key: String,
}

/// Rewrite every sound-alike of a dictionary entry in `text` to the entry's
/// own spelling. Returns the text unchanged when there is nothing to do.
pub fn apply_dictionary(text: &str, entries: &[String]) -> String {
    let mut parsed: Vec<Entry> = entries
        .iter()
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .filter_map(|e| {
            let words = tokenize(e);
            if words.is_empty() {
                return None;
            }
            let keys = words.iter().map(|w| phonetic_key(w)).collect();
            Some(Entry {
                text: e,
                words,
                keys,
            })
        })
        .collect();
    if parsed.is_empty() {
        return text.to_string();
    }
    // Longer entries first, so "Juno Bar" wins over "Juno" where both fit.
    parsed.sort_by_key(|e| std::cmp::Reverse(e.words.len()));

    let heard = words_of(text);
    let mut out = String::with_capacity(text.len());
    let mut copied_to = 0;
    let mut i = 0;
    while i < heard.len() {
        let Some((entry, span_end)) = parsed
            .iter()
            .find_map(|entry| matches_at(text, &heard, i, entry).map(|end| (entry, end)))
        else {
            i += 1;
            continue;
        };
        let start = heard[i].start;
        if &text[start..span_end] != entry.text {
            out.push_str(&text[copied_to..start]);
            out.push_str(entry.text);
            copied_to = span_end;
        }
        i += entry.words.len();
    }
    out.push_str(&text[copied_to..]);
    out
}

/// If `entry` is heard starting at word `i`, the byte offset where its last
/// word ends.
fn matches_at(text: &str, heard: &[Heard], i: usize, entry: &Entry) -> Option<usize> {
    let n = entry.words.len();
    if i + n > heard.len() {
        return None;
    }
    for (offset, (word, key)) in entry.words.iter().zip(&entry.keys).enumerate() {
        let h = &heard[i + offset];
        if !words_match(&h.lower, &h.key, word, key) {
            return None;
        }
        // The words of a phrase are adjacent in speech, so anything but
        // whitespace between them (a full stop, a comma) is a different
        // sentence, not the phrase.
        if offset > 0 {
            let gap = &text[heard[i + offset - 1].end..h.start];
            if !gap.chars().all(char::is_whitespace) {
                return None;
            }
        }
    }
    Some(heard[i + n - 1].end)
}

/// Split the text into words the same way the matcher does (runs of letters
/// and digits), keeping where each one sits so the rest of the text can be
/// copied through untouched.
fn words_of(text: &str) -> Vec<Heard> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for (idx, c) in text.char_indices() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(idx),
            (false, Some(s)) => {
                words.push(heard(text, s, idx));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        words.push(heard(text, s, text.len()));
    }
    words
}

fn heard(text: &str, start: usize, end: usize) -> Heard {
    let lower = text[start..end].to_lowercase();
    let key = phonetic_key(&lower);
    Heard {
        start,
        end,
        lower,
        key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn juno() -> Vec<String> {
        vec!["Juno".to_string()]
    }

    #[test]
    fn the_spellings_whisper_invents_for_juno_become_juno() {
        for heard in ["Juneau", "juneau", "joono", "Juneou", "JUNO", "juno"] {
            assert_eq!(
                apply_dictionary(&format!("ask {heard} about it"), &juno()),
                "ask Juno about it",
                "{heard}"
            );
        }
    }

    #[test]
    fn punctuation_spacing_and_possessives_are_kept() {
        assert_eq!(
            apply_dictionary("Hey, Juneau. Is that Juneau's bar?  yes", &juno()),
            "Hey, Juno. Is that Juno's bar?  yes"
        );
        assert_eq!(apply_dictionary("juneau", &juno()), "Juno");
        assert_eq!(apply_dictionary("", &juno()), "");
    }

    /// The wake word's own rule, inherited: similar words are not the word.
    #[test]
    fn different_words_are_left_alone() {
        let text = "judo in june with john and jane, you know";
        assert_eq!(apply_dictionary(text, &juno()), text);
    }

    #[test]
    fn a_text_that_already_matches_comes_back_unchanged() {
        let text = "Juno, open Safari.";
        assert_eq!(apply_dictionary(text, &juno()), text);
    }

    #[test]
    fn multi_word_entries_need_adjacent_words_and_beat_shorter_ones() {
        let entries = vec!["Juno".to_string(), "Juno Bar".to_string()];
        assert_eq!(
            apply_dictionary("open the juneau bar now", &entries),
            "open the Juno Bar now"
        );
        // Across a sentence boundary the phrase is two sentences.
        assert_eq!(
            apply_dictionary("call juneau. bar the door", &entries),
            "call Juno. bar the door"
        );
    }

    #[test]
    fn empty_and_blank_entries_do_nothing() {
        let text = "nothing to see";
        assert_eq!(apply_dictionary(text, &[]), text);
        assert_eq!(
            apply_dictionary(
                text,
                &["".to_string(), "   ".to_string(), "...".to_string()]
            ),
            text
        );
    }

    /// Entries are the person's spelling, so their case is what gets typed,
    /// and a non-ASCII entry does not split a character.
    #[test]
    fn entries_keep_their_own_case_and_survive_unicode() {
        let entries = vec!["Lacy".to_string(), "Zoë".to_string()];
        assert_eq!(
            apply_dictionary("lacey and zoe went out — très bien", &entries),
            "Lacy and Zoë went out — très bien"
        );
    }
}
