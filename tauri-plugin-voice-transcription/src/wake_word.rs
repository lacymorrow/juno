//! Wake phrase matching for always-listening mode.
//!
//! Whisper writes down what it thinks it heard, and what it writes for a name
//! it does not know is a guess at the spelling. "Juno" comes back as "Juneau",
//! with a capital and a full stop, and a literal comparison misses it even
//! though the person said exactly the right word. So the comparison is done on
//! how the two words sound, not on how they are spelled.
//!
//! ## Why not Soundex or Metaphone
//!
//! The usual phonetic algorithms throw the vowels away and keep the consonant
//! skeleton. That is right for surnames on a census form and wrong for a wake
//! word: "juno", "john", "jane" and "june" all reduce to the same consonant
//! skeleton, so an app listening for "juno" would start up every time someone
//! said a friend's name. A wake phrase that fires on a conversation it was not
//! invited into is a much worse failure than one that occasionally misses, so
//! the key below keeps the vowels and folds only the spellings of them.
//!
//! The key is a small Metaphone-style rewrite: English spelling patterns that
//! sound the same collapse to one symbol (ph/f, ck/k, ch, sh, th, silent
//! terminal e, silent word-initial k in "kn"), and each run of vowel letters
//! collapses to a single vowel class chosen by the usual English reading of
//! that run ("eau" in Juneau reads the same as the "o" in Juno, the same way it
//! does in beau and plateau). Nothing here names a specific wake word.
//!
//! ## What matches
//!
//! - `juno` and `juneau` both key to `jUnO`, so they match.
//! - `june` loses its silent e and keys to `jUn`, so it does not.
//! - `john` keys to `jOn` and `judo` to `jUdO`, so neither does.
//!
//! Above that sits a length-scaled edit distance, because a long word can
//! survive one wrong sound and still be unmistakable while a four-symbol word
//! cannot: one edit turns `jUnO` into `jUdO`, a different word entirely.

/// How many single-symbol differences between two phonetic keys still count as
/// the same word, given the length of the wake phrase's own key.
///
/// Short keys get nothing. At four or five symbols an edit is not a slip, it is
/// a different word: one symbol is all that separates `jUnO` from `jUdO` and
/// `jEnO`, which are judo and Geno. A key of six or more can spend one, because
/// a word that long stays unmistakable through a wrong sound, and because that
/// is where Whisper's spelling actually wanders (a plural, a swallowed
/// syllable). The cost of that one edit is stated in the tests: a phrase as
/// long as "computer" will also answer to "commuter".
fn allowed_edits(key_len: usize) -> usize {
    if key_len >= 6 {
        1
    } else {
        0
    }
}

/// Find the first configured wake phrase that the transcript contains.
///
/// Returns the configured phrase itself, lowercased and trimmed, never the
/// words that were heard: the app looks the returned phrase back up in the
/// trigger list to decide what the activation should drive, so it has to be a
/// phrase that list actually holds.
pub fn match_wake_phrase(transcript: &str, wake_phrases: &[String]) -> Option<String> {
    let heard: Vec<String> = tokenize(transcript);
    if heard.is_empty() {
        return None;
    }
    let heard_keys: Vec<String> = heard.iter().map(|w| phonetic_key(w.as_str())).collect();

    for phrase in wake_phrases {
        let phrase_tokens = tokenize(phrase);
        if phrase_tokens.is_empty() {
            continue;
        }
        let phrase_keys: Vec<String> = phrase_tokens
            .iter()
            .map(|w| phonetic_key(w.as_str()))
            .collect();

        if contains_phrase(&heard, &heard_keys, &phrase_tokens, &phrase_keys) {
            return Some(phrase.trim().to_lowercase());
        }
    }

    None
}

/// Whether the heard words contain the phrase's words, in order and adjacent.
///
/// Adjacency matters: "hey juno" should not be found in "hey, could you ask
/// juno about it", which is someone talking about Juno rather than to it.
fn contains_phrase(
    heard: &[String],
    heard_keys: &[String],
    phrase: &[String],
    phrase_keys: &[String],
) -> bool {
    if phrase.len() > heard.len() {
        return false;
    }
    (0..=(heard.len() - phrase.len())).any(|start| {
        (0..phrase.len()).all(|offset| {
            words_match(
                &heard[start + offset],
                &heard_keys[start + offset],
                &phrase[offset],
                &phrase_keys[offset],
            )
        })
    })
}

/// Whether one heard word is the wake phrase's word: spelled the same, or
/// sounding the same, or near enough for a word long enough to afford it.
fn words_match(heard: &str, heard_key: &str, phrase: &str, phrase_key: &str) -> bool {
    if heard == phrase {
        return true;
    }
    if phrase_key.is_empty() || heard_key.is_empty() {
        return false;
    }
    if heard_key == phrase_key {
        return true;
    }

    let budget = allowed_edits(phrase_key.chars().count());
    if budget == 0 {
        return false;
    }
    // A key that is already further away in length than the budget allows can
    // never come back under it, and skipping those keeps the quadratic part off
    // the hot path of every transcription.
    let heard_len = heard_key.chars().count();
    let phrase_len = phrase_key.chars().count();
    if heard_len.abs_diff(phrase_len) > budget {
        return false;
    }
    edit_distance(heard_key, phrase_key) <= budget
}

/// Split text into bare lowercase words, dropping everything that is not a
/// letter or a digit.
///
/// This is what strips the trailing full stop off "hey juneau." and the comma
/// out of "hey, juno", both of which defeated the old literal comparison on
/// their own, before any question of how the word sounds comes up.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// True for the five letters that are always vowels. `y` and `w` are handled
/// where they appear, since both are vowels in some positions and consonants in
/// others.
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// The vowel class a run of vowel letters reads as.
///
/// Uppercase symbols so a vowel can never be confused with a consonant in the
/// key. The table holds the English vowel runs that are read as one sound; any
/// run not in it takes the class of its first letter, which is the common case
/// for a single vowel.
fn vowel_class(run: &str) -> char {
    match run {
        "eau" | "oa" | "oe" | "au" | "aw" | "ow" | "oi" | "oy" => 'O',
        "ea" | "ee" | "ie" | "ei" => 'E',
        "oo" | "ou" | "ew" | "eu" | "ui" | "ue" => 'U',
        "ai" | "ay" | "ey" => 'A',
        _ => match run.chars().next() {
            Some('a') => 'A',
            Some('e') => 'E',
            Some('i') | Some('y') => 'I',
            Some('o') => 'O',
            Some('u') | Some('w') => 'U',
            _ => 'I',
        },
    }
}

/// Rewrite a word as roughly how it sounds: consonants as themselves with the
/// English spelling patterns folded together, vowel runs as one vowel class
/// each.
///
/// Works on `chars`, never on byte offsets, so a transcription that comes back
/// with an accent or a non-Latin character cannot split a character in half.
fn phonetic_key(word: &str) -> String {
    let mut chars: Vec<char> = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();

    if chars.is_empty() {
        return String::new();
    }

    // Silent first letters: "know", "gnome", "pneumatic", "write", "psalm".
    if chars.len() > 2 {
        let head: String = chars.iter().take(2).collect();
        if matches!(head.as_str(), "kn" | "gn" | "pn" | "wr" | "ps") {
            chars.remove(0);
        }
    }

    // A terminal e after a consonant is silent in English, which is the whole
    // difference between "june" and "juno" and worth keeping.
    if chars.len() > 2 {
        let last = chars.len() - 1;
        if chars[last] == 'e' && !is_vowel(chars[last - 1]) {
            chars.pop();
        }
    }

    let mut key = String::new();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let after = chars.get(i + 2).copied();
        let prev = if i > 0 {
            chars.get(i - 1).copied()
        } else {
            None
        };

        // A doubled consonant is one sound: "juno" and "junno" are the same word.
        if !is_vowel(c) && prev == Some(c) {
            i += 1;
            continue;
        }

        // Vowel runs, including a trailing y or w that belongs to the vowel
        // sound ("hey", "know"). A word-initial y is a consonant and is handled
        // below instead.
        if is_vowel(c) || (c == 'y' && i > 0) {
            let mut run = String::new();
            while i < n {
                let r = chars[i];
                let vowel_here = is_vowel(r)
                    || ((r == 'y' || r == 'w') && !run.is_empty())
                    || (r == 'y' && i > 0 && run.is_empty());
                if !vowel_here {
                    break;
                }
                run.push(r);
                i += 1;
            }
            key.push(vowel_class(&run));
            continue;
        }

        match c {
            // A terminal "mb" is one sound: "comb", "dumb".
            'b' => {
                let silent_after_m = i + 1 == n && prev == Some('m');
                if !silent_after_m {
                    key.push('b');
                }
            }
            'c' => match next {
                Some('h') => {
                    key.push('X');
                    i += 1;
                }
                Some('k') | Some('q') => {
                    key.push('k');
                    i += 1;
                }
                Some('e') | Some('i') | Some('y') => key.push('s'),
                _ => key.push('k'),
            },
            'd' => {
                if next == Some('g') && matches!(after, Some('e') | Some('i') | Some('y')) {
                    key.push('j');
                    i += 2;
                } else {
                    key.push('d');
                }
            }
            'g' => match next {
                // "night", "through": the h swallows the g.
                Some('h') => i += 1,
                Some('e') | Some('i') | Some('y') => key.push('j'),
                _ => key.push('g'),
            },
            // h is only its own sound at the start of a syllable. Inside a word
            // after a vowel it is silent, which is what keeps "john" as "jOn".
            'h' => {
                let starts_syllable = i == 0 || next.is_some_and(is_vowel);
                if starts_syllable {
                    key.push('h');
                }
            }
            'p' => {
                if next == Some('h') {
                    key.push('f');
                    i += 1;
                } else {
                    key.push('p');
                }
            }
            'q' => key.push('k'),
            's' => {
                if next == Some('h') {
                    key.push('X');
                    i += 1;
                } else {
                    key.push('s');
                }
            }
            't' => {
                if next == Some('h') {
                    key.push('0');
                    i += 1;
                } else {
                    key.push('t');
                }
            }
            'v' => key.push('f'),
            // A w before a vowel is a consonant; after one it was already eaten
            // by the vowel run above.
            'w' => {
                if next.is_some_and(is_vowel) {
                    key.push('w');
                }
            }
            'x' => key.push_str("ks"),
            'y' => key.push('y'),
            'z' => key.push('s'),
            other => key.push(other),
        }

        i += 1;
    }

    key
}

/// Levenshtein distance between two keys, counted in characters.
///
/// Two rows rather than a full matrix: these are single words, but this runs on
/// every transcription the always-listening loop produces.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            current[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut prev, &mut current);
    }

    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrases(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The phrase list the app actually builds for a trigger whose phrase is
    /// "juno": the bare word and its "hey" form.
    fn juno_phrases() -> Vec<String> {
        phrases(&["juno", "hey juno"])
    }

    #[test]
    fn juneau_is_juno() {
        assert_eq!(phonetic_key("juneau"), phonetic_key("juno"));
        assert_eq!(
            match_wake_phrase("juneau", &juno_phrases()),
            Some("juno".to_string())
        );
    }

    #[test]
    fn the_reported_transcription_matches() {
        // The exact line from the log: capitalised, with a trailing full stop.
        assert_eq!(
            match_wake_phrase("hey juneau.", &juno_phrases()),
            Some("juno".to_string())
        );
        assert_eq!(
            match_wake_phrase("Hey Juneau.", &juno_phrases()),
            Some("juno".to_string())
        );
    }

    #[test]
    fn punctuation_and_spacing_do_not_matter() {
        for heard in [
            "Juno",
            "juno.",
            "  juno  ",
            "Hey, Juno!",
            "hey    juno",
            "okay juno, open safari",
            "juno's",
        ] {
            assert_eq!(
                match_wake_phrase(heard, &juno_phrases()),
                Some("juno".to_string()),
                "expected a match in {heard:?}"
            );
        }
    }

    #[test]
    fn other_spellings_of_the_same_sound_match() {
        // More of Whisper's guesses at a name it does not know, all read the
        // same way aloud: a silent trailing h, a doubled consonant.
        for heard in ["junoh", "junno"] {
            assert_eq!(
                phonetic_key(heard),
                phonetic_key("juno"),
                "{heard:?} keyed to {:?}",
                phonetic_key(heard)
            );
            assert_eq!(
                match_wake_phrase(heard, &juno_phrases()),
                Some("juno".to_string())
            );
        }
    }

    #[test]
    fn near_misses_do_not_match() {
        // Ordinary speech that a consonant-only phonetic key would have woken
        // Juno on. Every one of these is a real thing a person says near a
        // laptop, and none of them is an invitation.
        let not_juno = [
            "john",
            "hey john",
            "john called me back",
            "june",
            "see you in june",
            "junior",
            "my junior dev",
            "jonah",
            "judo",
            "journal",
            "journey",
            "jupiter",
            "bruno",
            "bruno mars",
            "uno",
            "you know",
            "you know what i mean",
            "unknown",
            "who knew",
            "junk",
            "junior year",
            "geno",
            "jane",
            "gina",
            "dunno",
            "lunar",
            "tuna",
            "i do not know",
        ];
        for heard in not_juno {
            assert_eq!(
                match_wake_phrase(heard, &juno_phrases()),
                None,
                "{heard:?} must not wake Juno (key {:?})",
                phonetic_key(heard)
            );
        }
    }

    #[test]
    fn a_short_key_spends_no_edits() {
        // One edit is the whole difference between these words, so a four
        // symbol key gets no budget at all.
        assert_eq!(allowed_edits(phonetic_key("juno").chars().count()), 0);
        assert_ne!(phonetic_key("judo"), phonetic_key("juno"));
        assert_eq!(
            edit_distance(&phonetic_key("judo"), &phonetic_key("juno")),
            1
        );
        assert_eq!(match_wake_phrase("judo", &juno_phrases()), None);
    }

    #[test]
    fn a_long_key_survives_a_slip() {
        // "computer" is long enough that a plural or a dropped sound leaves no
        // doubt about which word it was.
        let computer = phrases(&["computer", "hey computer"]);
        assert_eq!(
            match_wake_phrase("hey computer.", &computer),
            Some("computer".to_string())
        );
        assert_eq!(
            match_wake_phrase("computers", &computer),
            Some("computer".to_string())
        );
        // The price of that one edit, written down rather than discovered: a
        // phrase this long answers to a word one sound away from it. Short
        // phrases, which is nearly all of them, spend nothing and so cannot do
        // this.
        assert!(match_wake_phrase("my commuter train", &computer).is_some());
        assert_eq!(
            match_wake_phrase("my commuter train", &juno_phrases()),
            None
        );
    }

    #[test]
    fn a_multi_word_phrase_needs_its_words_together() {
        let only_hey = phrases(&["hey juno"]);
        assert_eq!(
            match_wake_phrase("hey juno what is the weather", &only_hey),
            Some("hey juno".to_string())
        );
        // Talking about Juno rather than to it.
        assert_eq!(
            match_wake_phrase("hey, could you ask juno about it", &only_hey),
            None
        );
        assert_eq!(match_wake_phrase("juno", &only_hey), None);
    }

    #[test]
    fn the_configured_phrase_comes_back_not_the_heard_words() {
        // The app looks this string back up in its trigger list, so it has to
        // be the phrase as configured, never "juneau".
        assert_eq!(
            match_wake_phrase("hey juneau.", &phrases(&["hey juno"])),
            Some("hey juno".to_string())
        );
    }

    #[test]
    fn empty_input_matches_nothing() {
        assert_eq!(match_wake_phrase("", &juno_phrases()), None);
        assert_eq!(match_wake_phrase("...", &juno_phrases()), None);
        assert_eq!(match_wake_phrase("juno", &phrases(&[])), None);
        assert_eq!(match_wake_phrase("juno", &phrases(&["", "   "])), None);
    }

    #[test]
    fn blank_audio_markers_do_not_match() {
        // Whisper emits these for silence; they must never look like a phrase.
        assert_eq!(match_wake_phrase("[BLANK_AUDIO]", &juno_phrases()), None);
        assert_eq!(match_wake_phrase("[_SILENCE_]", &juno_phrases()), None);
    }

    #[test]
    fn spelling_patterns_that_sound_alike_fold_together() {
        assert_eq!(phonetic_key("phone"), phonetic_key("fone"));
        assert_eq!(phonetic_key("knight"), phonetic_key("night"));
        assert_eq!(phonetic_key("cat"), phonetic_key("kat"));
        // And ones that do not sound alike stay apart.
        assert_ne!(phonetic_key("john"), phonetic_key("juno"));
        assert_ne!(phonetic_key("june"), phonetic_key("juno"));
        assert_ne!(phonetic_key("jane"), phonetic_key("juno"));
    }
}
