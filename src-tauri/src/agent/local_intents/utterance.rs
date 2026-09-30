//! Utterance normalization shared by the non-media intents.
//!
//! The media grammar keeps its own, older normalizer (it drops filler words
//! anywhere, which is safe for its tiny closed vocabulary). The domains added
//! later need something stricter, because some of them capture a name ("open
//! Safari") and a loose normalizer would let a compound request collapse into
//! a bare command. This one only trims courtesy at the edges, and refuses the
//! utterance outright when a clause word shows up after the command starts.

/// Courtesy that may open a command: "hey Juno, could you please ...".
const LEADING_COURTESY: &[&str] = &[
    "hey", "hi", "hello", "ok", "okay", "juno", "please", "can", "could", "would", "will", "you",
    "u", "ahead", "and", "just", "kindly", "now", "yo",
];

/// Courtesy that may close a command: "... for me, thanks".
const TRAILING_COURTESY: &[&str] = &[
    "please", "thanks", "thank", "you", "now", "for", "me", "juno", "quickly", "real", "quick",
];

/// Words that join a second clause. Any of these after the command has
/// started means the request has more in it than one native call can serve,
/// so the whole utterance goes to the agent.
const CLAUSE_WORDS: &[&str] = &[
    "and", "then", "also", "after", "before", "when", "while", "if", "but", "so", "because",
    "until", "or", "plus", "unless", "once",
];

/// Articles and possessives that never change which command was meant.
const DROPPED_ANYWHERE: &[&str] = &["the", "my", "please"];

/// Units a leading "a"/"an" can quantify ("for a minute" -> "for 1 minute").
const UNIT_WORDS: &[&str] = &[
    "second", "seconds", "sec", "secs", "minute", "minutes", "min", "mins", "hour", "hours", "hr",
    "hrs",
];

/// Normalize a raw query into a canonical, single-clause command string, or
/// `None` when it is empty or compound.
///
/// Lower-cases, turns punctuation into spaces (keeping `.` inside words for
/// domains and `%` for "50%"), expands the handful of contractions the
/// grammars use, trims courtesy from both ends, rejects clause words, and
/// drops articles.
pub fn normalize(query: &str) -> Option<String> {
    let lower = query
        .to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace("half an hour", "30 minutes")
        .replace("half a minute", "30 seconds");
    let spaced: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() || c == '\'' || c == '.' || c == '%' {
                c
            } else {
                ' '
            }
        })
        .collect();

    let mut tokens: Vec<String> = Vec::new();
    for raw in spaced.split_whitespace() {
        let word = raw.trim_matches(|c| c == '.' || c == '\'');
        let expanded = match word {
            "what's" | "whats" => "what is",
            "it's" => "it is",
            "how's" => "how is",
            "where's" => "where is",
            "i'm" => "i am",
            "today's" | "todays" => "today",
            other => other,
        };
        for part in expanded.split(' ') {
            let part = part.replace('\'', "");
            if !part.is_empty() {
                tokens.push(part);
            }
        }
    }

    // "go" is courtesy only in "go ahead"; "go to github.com" is a command.
    let start = (0..tokens.len()).find(|&i| {
        let t = tokens[i].as_str();
        let go_ahead = t == "go" && tokens.get(i + 1).is_some_and(|n| n == "ahead");
        !go_ahead && !LEADING_COURTESY.contains(&t)
    })?;
    let mut end = tokens.len();
    while end > start && TRAILING_COURTESY.contains(&tokens[end - 1].as_str()) {
        end -= 1;
    }
    let body = tokens.get(start..end)?;
    if body.is_empty() || body.iter().any(|t| CLAUSE_WORDS.contains(&t.as_str())) {
        return None;
    }

    let mut out: Vec<&str> = Vec::with_capacity(body.len());
    for (i, token) in body.iter().enumerate() {
        let t = token.as_str();
        if DROPPED_ANYWHERE.contains(&t) {
            continue;
        }
        if t == "a" || t == "an" {
            let next_is_unit = body
                .get(i + 1)
                .is_some_and(|n| UNIT_WORDS.contains(&n.as_str()));
            if next_is_unit {
                out.push("1");
            }
            continue;
        }
        out.push(t);
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(" "))
    }
}

/// Read a spoken or written whole number: "5", "five", "twenty".
pub fn number_value(token: &str) -> Option<u32> {
    if let Ok(n) = token.parse::<u32>() {
        return Some(n);
    }
    let n = match token {
        "zero" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        "hundred" => 100,
        _ => return None,
    };
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_courtesy_and_punctuation() {
        assert_eq!(
            normalize("Hey Juno, could you please open Safari?").as_deref(),
            Some("open safari")
        );
        assert_eq!(
            normalize("Go ahead and lock the screen, thanks!").as_deref(),
            Some("lock screen")
        );
        assert_eq!(
            normalize("go to github.com").as_deref(),
            Some("go to github.com")
        );
        assert_eq!(
            normalize("What\u{2019}s the time now?").as_deref(),
            Some("what is time")
        );
        assert_eq!(
            normalize("open GitHub.com.").as_deref(),
            Some("open github.com")
        );
        assert_eq!(
            normalize("set the volume to 50%").as_deref(),
            Some("set volume to 50%")
        );
    }

    #[test]
    fn a_before_a_unit_becomes_one() {
        assert_eq!(
            normalize("set a timer for a minute").as_deref(),
            Some("set timer for 1 minute")
        );
        assert_eq!(
            normalize("set a timer for half an hour").as_deref(),
            Some("set timer for 30 minutes")
        );
    }

    #[test]
    fn compound_requests_are_refused() {
        for q in [
            "open Safari and find me flights to Denver",
            "open Safari then go to github",
            "set a timer for 5 minutes and remind me to stretch",
            "lock the screen after the build finishes",
            "turn the volume up if a call comes in",
            "open Slack or Discord",
        ] {
            assert_eq!(normalize(q), None, "{:?} must reach the agent", q);
        }
    }

    #[test]
    fn empty_and_pure_courtesy_are_nothing() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize("hey juno"), None);
        assert_eq!(normalize("please, thank you"), None);
    }

    #[test]
    fn number_words() {
        assert_eq!(number_value("5"), Some(5));
        assert_eq!(number_value("five"), Some(5));
        assert_eq!(number_value("fifty"), Some(50));
        assert_eq!(number_value("lots"), None);
    }
}
