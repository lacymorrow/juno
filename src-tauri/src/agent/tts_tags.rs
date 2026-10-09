//! Shared `<TTS>` tag extraction for every AI provider.
//!
//! The system prompt asks the model to put the spoken channel inside
//! `<TTS>...</TTS>` and everything else outside it. Each provider hands its
//! text through here so the spoken text reaches the speaker and the display
//! text reaches the chat with the tags removed. Keeping the parser in one place
//! means a provider cannot forget to speak (LAC: the Claude CLI provider never
//! extracted the tags, so nothing was spoken and raw tags showed in the chat).
//!
//! The same pass guards the component channel. A component tag whose name is
//! still arriving (`<Weath`, `</Car`) is held back until the next character
//! says what it is, so a half-typed tag name never reaches the chat as raw
//! text. Once the name resolves the markup goes out and the chat renders it
//! (or a placeholder while its attributes are still streaming).
//!
//! Inside an open `<TTS>` block the spoken text is released one sentence at a
//! time, as soon as the sentence boundary arrives, so the person hears Juno
//! start answering while the rest of the reply is still being written. See
//! [`split_ready_sentences`] for the boundary rules.
//!
//! What is spoken is exactly what sits inside `<TTS>...</TTS>`, once, and
//! nothing else. Three guards keep a malformed reply from talking past its
//! spoken line: a close tag in any case (`</tts>`) closes the block, a stray
//! `<TTS>` inside an open block is dropped rather than read aloud, and a
//! provider calls [`TtsTagStream::end_block`] when a content block ends, so a
//! block the model never closed stops there instead of reading out the rest of
//! the reply.
//!
//! Two entry points:
//! - [`TtsTagStream`] for streaming providers. Feed chunks as they arrive; tags
//!   split across chunk boundaries are held back until they resolve.
//! - [`split_tts_tags`] for whole strings (non-streaming providers and the
//!   runner's final safety net).

const OPEN_TAG: &str = "<TTS>";
const CLOSE_TAG: &str = "</TTS>";

/// Longest component-tag fragment worth holding back. Real component names
/// are short; anything longer is prose and goes out rather than stalling the
/// stream.
const MAX_COMPONENT_FRAGMENT_BYTES: usize = 64;

/// True when `rest` starts with `tag`, ignoring ASCII case.
fn starts_with_tag(rest: &str, tag: &str) -> bool {
    rest.len() >= tag.len() && rest.as_bytes()[..tag.len()].eq_ignore_ascii_case(tag.as_bytes())
}

/// True when all of `rest` is a proper prefix of `tag`, ignoring ASCII case:
/// the tag may still be arriving.
fn is_partial_tag(rest: &str, tag: &str) -> bool {
    rest.len() < tag.len() && tag.as_bytes()[..rest.len()].eq_ignore_ascii_case(rest.as_bytes())
}

/// True when `rest` is the start of a component tag whose name has not
/// finished arriving: `<`, `</`, `<W`, `<WeatherCa`, `</Card`. It must run to
/// the end of the buffer; the next character decides whether it is a tag.
/// Lowercase names (`<br`) are HTML or prose and are not held.
fn is_unresolved_component_fragment(rest: &str) -> bool {
    if rest.len() > MAX_COMPONENT_FRAGMENT_BYTES {
        return false;
    }
    let Some(after_lt) = rest.strip_prefix('<') else {
        return false;
    };
    let name = after_lt.strip_prefix('/').unwrap_or(after_lt);
    let mut chars = name.chars();
    match chars.next() {
        // `<` or `</` alone: wait for the next character.
        None => true,
        Some(first) => first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_alphanumeric()),
    }
}

/// A spoken chunk shorter than this is merged into the sentence after it, so
/// "Sure. Opening it now." goes out as one breath instead of two clipped ones.
const MIN_SPOKEN_CHUNK_CHARS: usize = 20;

/// Words that end in a period without ending the sentence. Compared lowercase,
/// without the final period. Initials and dotted forms like `U.S` are handled
/// by [`is_abbreviation`] itself.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "e.g", "i.e", "inc", "ltd", "approx",
    "dept", "fig", "vol", "mt", "gen", "sen", "rep", "capt", "col", "lt", "sgt",
];

/// True when `before` (the text ahead of a `.`) ends in a token that is an
/// abbreviation, an initial ("J."), or a dotted form ("U.S.").
fn is_abbreviation(before: &str) -> bool {
    let token = before
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or("")
        .trim_start_matches(|c: char| {
            matches!(c, '(' | '[' | '"' | '\'' | '\u{201c}' | '\u{2018}')
        });
    if token.is_empty() {
        return false;
    }
    let lower = token.to_lowercase();
    if ABBREVIATIONS.contains(&lower.as_str()) {
        return true;
    }
    // A lone capital letter is an initial.
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_alphabetic() && c.is_uppercase();
    }
    // "U.S", "a.m", "p.m": every dot-separated piece is one letter.
    token.contains('.')
        && token
            .split('.')
            .all(|piece| piece.chars().count() == 1 && piece.chars().all(char::is_alphabetic))
}

/// Pull every finished sentence off the front of `text`.
///
/// Returns the sentences (trimmed) and the byte offset where the unfinished
/// remainder starts. A sentence ends at `.`, `!` or `?` (closing quotes and
/// brackets allowed after it) that is followed by whitespace; punctuation at
/// the very end of `text` waits, because the next character decides whether it
/// is a boundary. Not boundaries: ellipses, abbreviations and initials, and
/// decimals like `3.5` (no whitespace follows the point). A sentence under
/// `min_chars` is held and merged into the one after it.
///
/// The result depends only on the text, never on how it was chunked, so the
/// streaming path and the whole-string path always agree.
pub fn split_ready_sentences(text: &str, min_chars: usize) -> (Vec<String>, usize) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let is_end = |c: char| matches!(c, '.' | '!' | '?' | '\u{2026}');
    let is_closer = |c: char| matches!(c, '"' | '\'' | '\u{201d}' | '\u{2019}' | ')' | ']');
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if !is_end(chars[i].1) {
            i += 1;
            continue;
        }
        let mut run_end = i;
        while run_end + 1 < chars.len() && is_end(chars[run_end + 1].1) {
            run_end += 1;
        }
        let mut close = run_end;
        while close + 1 < chars.len() && is_closer(chars[close + 1].1) {
            close += 1;
        }
        if close + 1 >= chars.len() {
            // Punctuation at the very end: wait for the next character.
            break;
        }
        if !chars[close + 1].1.is_whitespace() {
            i = close + 1;
            continue;
        }
        let run = &chars[i..=run_end];
        let ellipsis = run.iter().filter(|(_, c)| *c == '.').count() >= 2
            || run.iter().any(|(_, c)| *c == '\u{2026}');
        let abbreviation =
            run.len() == 1 && run[0].1 == '.' && is_abbreviation(&text[start..run[0].0]);
        if !ellipsis && !abbreviation {
            let end = chars[close].0 + chars[close].1.len_utf8();
            let candidate = text[start..end].trim();
            if candidate.chars().count() >= min_chars {
                sentences.push(candidate.to_string());
                let mut next = close + 1;
                while next < chars.len() && chars[next].1.is_whitespace() {
                    next += 1;
                }
                start = if next < chars.len() {
                    chars[next].0
                } else {
                    text.len()
                };
                i = next;
                continue;
            }
        }
        i = close + 1;
    }
    (sentences, start)
}

/// Incremental `<TTS>` parser for streamed text.
///
/// `push` returns the display text that is safe to show now plus every
/// spoken chunk that completed inside this chunk: each finished sentence of an
/// open `<TTS>` block, and the rest of the block when it closes. Characters that could
/// be the start of a tag are kept in the buffer until the next chunk (or
/// `finish`) decides what they are.
#[derive(Debug, Default)]
pub struct TtsTagStream {
    /// Unconsumed tail of the input (a possible partial tag).
    buffer: String,
    /// True while between `<TTS>` and `</TTS>`.
    in_tag: bool,
    /// Spoken text collected since the last `<TTS>`.
    spoken: String,
}

impl TtsTagStream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Move every finished sentence out of the open block's text.
    fn release_sentences(&mut self, out: &mut Vec<String>) {
        let (sentences, consumed) = split_ready_sentences(&self.spoken, MIN_SPOKEN_CHUNK_CHARS);
        if consumed > 0 {
            self.spoken.drain(..consumed);
        }
        out.extend(sentences);
    }

    /// Release what is left of a block that just closed (or ran out).
    fn release_remainder(&mut self, out: &mut Vec<String>) {
        self.release_sentences(out);
        let rest = std::mem::take(&mut self.spoken);
        let rest = rest.trim();
        if !rest.is_empty() {
            out.push(rest.to_string());
        }
    }

    /// Feed one chunk. Returns `(display_text, spoken_blocks)`.
    pub fn push(&mut self, chunk: &str) -> (String, Vec<String>) {
        self.buffer.push_str(chunk);

        let mut display = String::new();
        let mut spoken_blocks = Vec::new();
        // Byte offset of the first unconsumed char. Only advanced at char
        // boundaries, so slicing the buffer at it is safe.
        let mut consumed = 0;

        loop {
            let rest = &self.buffer[consumed..];
            if rest.is_empty() {
                break;
            }

            if self.in_tag {
                // Any case: a model that closes with `</tts>` must not have
                // the rest of its reply read out.
                if starts_with_tag(rest, CLOSE_TAG) {
                    self.release_remainder(&mut spoken_blocks);
                    self.in_tag = false;
                    consumed += CLOSE_TAG.len();
                    continue;
                }
                if rest.starts_with(OPEN_TAG) {
                    // A nested open tag is markup, not speech.
                    consumed += OPEN_TAG.len();
                    continue;
                }
                if is_partial_tag(rest, CLOSE_TAG) || OPEN_TAG.starts_with(rest) {
                    // Possible partial tag at the end; wait for more.
                    break;
                }
            } else {
                if rest.starts_with(OPEN_TAG) {
                    self.in_tag = true;
                    consumed += OPEN_TAG.len();
                    continue;
                }
                if OPEN_TAG.starts_with(rest) {
                    // Possible partial "<TTS>" at the end; wait for more.
                    break;
                }
                if is_unresolved_component_fragment(rest) {
                    // Possible component tag whose name is still arriving.
                    break;
                }
            }

            // A plain character: route it to the channel we are in.
            match rest.chars().next() {
                Some(ch) => {
                    if self.in_tag {
                        self.spoken.push(ch);
                    } else {
                        display.push(ch);
                    }
                    consumed += ch.len_utf8();
                }
                None => break,
            }
        }

        self.buffer.drain(..consumed);
        if self.in_tag {
            self.release_sentences(&mut spoken_blocks);
        }
        (display, spoken_blocks)
    }

    /// The content block the text came from has ended (a tool call, a new
    /// text block, the end of the message). A spoken block cannot span two
    /// content blocks, so one still open was never closed: its text is
    /// released now and the parser leaves it, so the next block's text is
    /// shown rather than spoken. A half-arrived tag is dropped. Returns the
    /// spoken chunks released; display state is untouched.
    pub fn end_block(&mut self) -> Vec<String> {
        let mut spoken_blocks = Vec::new();
        if self.in_tag {
            // Inside a block the buffer only ever holds a partial tag.
            self.buffer.clear();
            self.release_remainder(&mut spoken_blocks);
            self.in_tag = false;
        }
        spoken_blocks
    }

    /// Flush at end of stream. Returns `(display_text, spoken_blocks)`.
    ///
    /// A block that never closed is still spoken (the model ran out of tokens
    /// or the tag was malformed), without the half-arrived tag that may end
    /// it; outside a block a partial tag left in the buffer is shown as plain
    /// text so nothing is silently dropped.
    pub fn finish(&mut self) -> (String, Vec<String>) {
        if self.in_tag {
            return (String::new(), self.end_block());
        }
        (std::mem::take(&mut self.buffer), Vec::new())
    }
}

/// Split a complete string into `(display_text, spoken_blocks)`.
///
/// Display text has every `<TTS>...</TTS>` block removed and is trimmed. An
/// unterminated block is spoken and removed from display.
pub fn split_tts_tags(text: &str) -> (String, Vec<String>) {
    if !text.contains('<') {
        return (text.trim().to_string(), Vec::new());
    }
    let mut stream = TtsTagStream::new();
    let (mut display, mut spoken) = stream.push(text);
    let (tail_display, tail_spoken) = stream.finish();
    display.push_str(&tail_display);
    spoken.extend(tail_spoken);
    (display.trim().to_string(), spoken)
}

/// True if the text still carries a `<TTS>` or `</TTS>` marker.
pub fn contains_tts_tags(text: &str) -> bool {
    text.contains(OPEN_TAG) || text.contains(CLOSE_TAG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_string_extracts_and_strips() {
        let (display, spoken) =
            split_tts_tags("<TTS>The moon is far.</TTS>\n\n- 239,000 miles\n- tidally locked");
        assert_eq!(spoken, vec!["The moon is far."]);
        assert_eq!(display, "- 239,000 miles\n- tidally locked");
    }

    #[test]
    fn whole_string_without_tags_is_untouched() {
        let (display, spoken) = split_tts_tags("plain text, no tags");
        assert_eq!(display, "plain text, no tags");
        assert!(spoken.is_empty());
    }

    #[test]
    fn multiple_blocks_in_one_string() {
        let (display, spoken) = split_tts_tags("<TTS>One.</TTS> middle <TTS>Two.</TTS> end");
        assert_eq!(spoken, vec!["One.", "Two."]);
        assert_eq!(display, "middle  end");
    }

    #[test]
    fn unterminated_block_is_spoken_not_shown() {
        let (display, spoken) = split_tts_tags("<TTS>Cut off mid sentence");
        assert_eq!(spoken, vec!["Cut off mid sentence"]);
        assert_eq!(display, "");
    }

    #[test]
    fn empty_block_is_dropped() {
        let (display, spoken) = split_tts_tags("<TTS>   </TTS>Hello");
        assert!(spoken.is_empty());
        assert_eq!(display, "Hello");
    }

    #[test]
    fn tts_only_response_yields_empty_display() {
        let (display, spoken) = split_tts_tags("<TTS>Paused.</TTS>");
        assert_eq!(spoken, vec!["Paused."]);
        assert_eq!(display, "");
    }

    #[test]
    fn streaming_tag_split_across_chunks() {
        let mut s = TtsTagStream::new();
        let (d1, t1) = s.push("Hi <T");
        assert_eq!(d1, "Hi ");
        assert!(t1.is_empty());
        let (d2, t2) = s.push("TS>spoken</TT");
        assert_eq!(d2, "");
        assert!(t2.is_empty());
        let (d3, t3) = s.push("S> shown");
        assert_eq!(d3, " shown");
        assert_eq!(t3, vec!["spoken"]);
        let (d4, t4) = s.finish();
        assert_eq!(d4, "");
        assert!(t4.is_empty());
    }

    #[test]
    fn streaming_one_char_at_a_time() {
        let input = "a<TTS>bc</TTS>d";
        let mut s = TtsTagStream::new();
        let mut display = String::new();
        let mut spoken = Vec::new();
        for ch in input.chars() {
            let (d, t) = s.push(&ch.to_string());
            display.push_str(&d);
            spoken.extend(t);
        }
        let (d, t) = s.finish();
        display.push_str(&d);
        spoken.extend(t);
        assert_eq!(display, "ad");
        assert_eq!(spoken, vec!["bc"]);
    }

    #[test]
    fn streaming_lone_angle_bracket_is_shown_once_resolved() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("a < b");
        assert_eq!(d1, "a < b");
        let (d2, _) = s.push("<");
        assert_eq!(d2, "");
        let (d3, _) = s.push("br>");
        assert_eq!(d3, "<br>");
    }

    #[test]
    fn streaming_partial_tag_at_end_is_flushed_as_text() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("done <TT");
        assert_eq!(d1, "done ");
        let (d2, t2) = s.finish();
        assert_eq!(d2, "<TT");
        assert!(t2.is_empty());
    }

    #[test]
    fn streaming_unterminated_block_spoken_on_finish() {
        let mut s = TtsTagStream::new();
        let (d1, t1) = s.push("<TTS>Still talk");
        assert_eq!(d1, "");
        assert!(t1.is_empty());
        let (d2, t2) = s.push("ing");
        assert_eq!(d2, "");
        assert!(t2.is_empty());
        let (d3, t3) = s.finish();
        assert_eq!(d3, "");
        assert_eq!(t3, vec!["Still talking"]);
    }

    #[test]
    fn streaming_multibyte_text_is_safe() {
        let mut s = TtsTagStream::new();
        let (d, t) = s.push("héllo <TTS>café ☕ 日本</TTS> wörld");
        assert_eq!(d, "héllo  wörld");
        assert_eq!(t, vec!["café ☕ 日本"]);
    }

    #[test]
    fn contains_tags_detects_either_marker() {
        assert!(contains_tts_tags("<TTS>x"));
        assert!(contains_tts_tags("x</TTS>"));
        assert!(!contains_tts_tags("<tts>lower</tts>"));
        assert!(!contains_tts_tags("none"));
    }

    #[test]
    fn spoken_block_is_released_by_the_chunk_that_closes_it() {
        // Speak-first: the acknowledgement must reach the speaker the moment
        // `</TTS>` lands, with nothing after it and before any tool call.
        let mut s = TtsTagStream::new();
        let (d1, t1) = s.push("<TTS>Sure, making that spreadsheet now.</TT");
        assert_eq!(d1, "");
        assert!(t1.is_empty());
        let (d2, t2) = s.push("S>");
        assert_eq!(d2, "");
        assert_eq!(t2, vec!["Sure, making that spreadsheet now."]);
    }

    #[test]
    fn open_tts_tag_split_mid_name_still_speaks() {
        let mut s = TtsTagStream::new();
        let mut spoken = Vec::new();
        let mut display = String::new();
        for chunk in ["<", "T", "TS>On it", ".</", "TTS>", "\n\nDone."] {
            let (d, t) = s.push(chunk);
            display.push_str(&d);
            spoken.extend(t);
        }
        assert_eq!(spoken, vec!["On it."]);
        assert_eq!(display, "\n\nDone.");
    }

    #[test]
    fn component_name_split_across_chunks_is_held_until_resolved() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("Here you go.\n<Weath");
        assert_eq!(d1, "Here you go.\n");
        let (d2, _) = s.push("erCard temp={5");
        assert_eq!(d2, "<WeatherCard temp={5");
        let (d3, _) = s.push("4} />");
        assert_eq!(d3, "4} />");
    }

    #[test]
    fn closing_component_tag_split_across_chunks_is_held() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("<Card>hello</Ca");
        assert_eq!(d1, "<Card>hello");
        let (d2, _) = s.push("rd>");
        assert_eq!(d2, "</Card>");
    }

    #[test]
    fn lone_angle_bracket_and_slash_wait_for_the_next_char() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("x </");
        assert_eq!(d1, "x ");
        let (d2, _) = s.push(" y");
        assert_eq!(d2, "</ y");
    }

    #[test]
    fn lowercase_and_prose_angle_brackets_are_not_held() {
        let mut s = TtsTagStream::new();
        let (d, _) = s.push("a <b");
        assert_eq!(d, "a <b");
        let (d, _) = s.push(" and 3 <4");
        assert_eq!(d, " and 3 <4");
    }

    #[test]
    fn overlong_fragment_is_released_as_text() {
        let mut s = TtsTagStream::new();
        let long = format!("<{}", "A".repeat(80));
        let (d, _) = s.push(&long);
        assert_eq!(d, long);
    }

    #[test]
    fn held_component_fragment_is_flushed_on_finish() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("cut off <Stat");
        assert_eq!(d1, "cut off ");
        let (d2, t2) = s.finish();
        assert_eq!(d2, "<Stat");
        assert!(t2.is_empty());
    }

    #[test]
    fn component_then_tts_in_one_chunk() {
        let mut s = TtsTagStream::new();
        let (d, t) = s.push("<Stat value={1} label=\"Rows\" /><TTS>Done.</TTS>");
        assert_eq!(d, "<Stat value={1} label=\"Rows\" />");
        assert_eq!(t, vec!["Done."]);
    }

    #[test]
    fn tts_then_component_split_between_chunks() {
        let mut s = TtsTagStream::new();
        let (d1, t1) = s.push("<TTS>Playing.</TTS>\n<Now");
        assert_eq!(d1, "\n");
        assert_eq!(t1, vec!["Playing."]);
        let (d2, t2) = s.push("PlayingCard app=\"Spotify\" />");
        assert_eq!(d2, "<NowPlayingCard app=\"Spotify\" />");
        assert!(t2.is_empty());
    }

    #[test]
    fn component_markup_inside_tts_is_not_held_or_shown() {
        // The prompt forbids nesting; if the model does it anyway the markup
        // stays on the spoken channel and never reaches the chat.
        let mut s = TtsTagStream::new();
        let (d1, t1) = s.push("<TTS>Look <Ca");
        assert_eq!(d1, "");
        assert!(t1.is_empty());
        let (d2, t2) = s.push("rd>here</Card></TTS>after");
        assert_eq!(d2, "after");
        assert_eq!(t2, vec!["Look <Card>here</Card>"]);
    }

    #[test]
    fn nested_components_stream_through_unchanged() {
        let input =
            "<AnimatedCard animation=\"fade-up\"><div><Stat value={72} /></div></AnimatedCard>";
        let mut s = TtsTagStream::new();
        let mut display = String::new();
        for ch in input.chars() {
            let (d, t) = s.push(&ch.to_string());
            assert!(t.is_empty());
            display.push_str(&d);
        }
        let (d, _) = s.finish();
        display.push_str(&d);
        assert_eq!(display, input);
    }

    #[test]
    fn stray_close_tag_outside_a_block_is_display_text() {
        let (display, spoken) = split_tts_tags("oops </TTS> fine");
        assert!(spoken.is_empty());
        assert_eq!(display, "oops </TTS> fine");
    }

    #[test]
    fn multibyte_text_before_a_split_component_is_safe() {
        let mut s = TtsTagStream::new();
        let (d1, _) = s.push("日本 ☕ <Wea");
        assert_eq!(d1, "日本 ☕ ");
        let (d2, _) = s.push("therCard />");
        assert_eq!(d2, "<WeatherCard />");
    }
    fn stream_all(chunks: &[&str]) -> (String, Vec<String>) {
        let mut s = TtsTagStream::new();
        let mut display = String::new();
        let mut spoken = Vec::new();
        for chunk in chunks {
            let (d, t) = s.push(chunk);
            display.push_str(&d);
            spoken.extend(t);
        }
        let (d, t) = s.finish();
        display.push_str(&d);
        spoken.extend(t);
        (display, spoken)
    }

    #[test]
    fn first_sentence_is_released_before_the_block_closes() {
        let mut s = TtsTagStream::new();
        let (_, t1) = s.push("<TTS>Sure, I can open that for you. Let me ");
        assert_eq!(t1, vec!["Sure, I can open that for you."]);
        let (_, t2) = s.push("find the right window now.");
        assert!(t2.is_empty());
        let (_, t3) = s.push("</TTS>");
        assert_eq!(t3, vec!["Let me find the right window now."]);
    }

    #[test]
    fn terminator_at_the_end_of_a_chunk_waits_for_the_next_character() {
        let mut s = TtsTagStream::new();
        let (_, t1) = s.push("<TTS>This is the first sentence.");
        assert!(t1.is_empty());
        let (_, t2) = s.push(" And");
        assert_eq!(t2, vec!["This is the first sentence."]);
    }

    #[test]
    fn sentence_split_mid_word_across_chunks() {
        let (_, spoken) = stream_all(&[
            "<TTS>The weather in Char",
            "lotte is sunny today.",
            " Expect a high of sev",
            "enty degrees.</TTS>",
        ]);
        assert_eq!(
            spoken,
            vec![
                "The weather in Charlotte is sunny today.",
                "Expect a high of seventy degrees."
            ]
        );
    }

    #[test]
    fn sentences_split_across_tag_boundaries() {
        let (display, spoken) = stream_all(&[
            "<TTS>Opening the spreadsheet now. Gi",
            "ve me a second.</TT",
            "S>\nHere it is.\n<TT",
            "S>All done, anything else?</TTS>",
        ]);
        assert_eq!(
            spoken,
            vec![
                "Opening the spreadsheet now.",
                "Give me a second.",
                "All done, anything else?"
            ]
        );
        assert_eq!(display, "\nHere it is.\n");
    }

    #[test]
    fn one_character_at_a_time_matches_whole_string() {
        let input = "<TTS>Okay, checking the calendar. You are free at 3.5 hours out! Dr. Smith e.g. is late.</TTS>tail";
        let chars: Vec<String> = input.chars().map(|c| c.to_string()).collect();
        let refs: Vec<&str> = chars.iter().map(String::as_str).collect();
        let (display, streamed) = stream_all(&refs);
        let (whole_display, whole) = split_tts_tags(input);
        assert_eq!(streamed, whole);
        assert_eq!(display, whole_display);
        assert_eq!(display, "tail");
    }

    #[test]
    fn short_sentences_merge_into_the_next_one() {
        let (_, spoken) = stream_all(&["<TTS>Sure. Opening it now. Back in a moment.</TTS>"]);
        assert_eq!(spoken, vec!["Sure. Opening it now.", "Back in a moment."]);
    }

    #[test]
    fn a_short_trailing_sentence_still_speaks_on_close() {
        let (_, spoken) = stream_all(&["<TTS>I finished the whole report for you. Done.</TTS>"]);
        assert_eq!(
            spoken,
            vec!["I finished the whole report for you.", "Done."]
        );
    }

    #[test]
    fn abbreviations_decimals_and_ellipses_do_not_split() {
        let (_, spoken) = stream_all(&[
            "<TTS>Dr. Jones lives in the U.S. and pays 3.5 percent, e.g. in tax. Well... maybe not today.</TTS>",
        ]);
        assert_eq!(
            spoken,
            vec![
                "Dr. Jones lives in the U.S. and pays 3.5 percent, e.g. in tax.",
                "Well... maybe not today."
            ]
        );
    }

    #[test]
    fn question_and_exclamation_split_and_quotes_stay_attached() {
        let (_, spoken) = stream_all(&[
            "<TTS>Did you say \"open the door\"? Great, I will do it right away!</TTS>",
        ]);
        assert_eq!(
            spoken,
            vec![
                "Did you say \"open the door\"?",
                "Great, I will do it right away!"
            ]
        );
    }

    #[test]
    fn split_ready_sentences_reports_the_remainder() {
        let (sentences, rest) = split_ready_sentences("First sentence here. Second one is ", 20);
        assert_eq!(sentences, vec!["First sentence here."]);
        assert_eq!(rest, "First sentence here. ".len());
    }

    /// Whitespace-normalized.
    fn norm(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// What should be spoken, worked out independently of the parser: the
    /// text after each `<TTS>` up to the next close tag in any case (or the
    /// end), with nested open tags removed.
    fn reference_spoken(input: &str) -> String {
        let mut out = String::new();
        let mut rest = input;
        while let Some(open) = rest.find(OPEN_TAG) {
            let after = &rest[open + OPEN_TAG.len()..];
            // ASCII lowercasing keeps byte offsets, so `end` indexes `after`.
            let end = after.to_ascii_lowercase().find("</tts>");
            let inner = end.map_or(after, |e| &after[..e]);
            out.push_str(&inner.replace(OPEN_TAG, " "));
            out.push(' ');
            rest = end.map_or("", |e| &after[e + CLOSE_TAG.len()..]);
        }
        norm(&out)
    }

    const INVARIANT_INPUTS: &[&str] = &[
        "Intro <TTS>Sure, opening Ghostty settings now. It takes a second.</TTS>\n\n- one\n- two <TTS>Done. Anything else you need?</TTS> tail.",
        "<TTS>Dr. Smith pays 3.5 percent, e.g. in tax... See https://example.com/a.b now. Okay!</TTS>shown after",
        "<TTS>This block never closes. And it keeps going to the end",
        "<TTS>Outer words <TTS>inner words here.</TTS> after </TTS> end",
        "<TTS>Spoken part is right here.</tts> Display text that must stay silent. More of it.",
        "h\u{e9}llo <TTS>caf\u{e9} \u{2615} \u{65e5}\u{672c} is great today. Second sentence.</TTS> w\u{f6}rld",
        "<Card>x</Card><TTS>Paused.</TTS><TTS>Two blocks, back to back.</TTS>",
    ];

    fn spoken_for(chunks: &[&str]) -> Vec<String> {
        stream_all(chunks).1
    }

    #[test]
    fn spoken_text_is_exactly_the_tagged_text_at_every_split() {
        for &input in INVARIANT_INPUTS {
            let expected = reference_spoken(input);
            let whole = spoken_for(&[input]);
            assert_eq!(norm(&whole.join(" ")), expected, "whole: {input:?}");
            assert!(whole.iter().all(|c| !c.trim().is_empty()));
            let bounds: Vec<usize> = (1..input.len())
                .filter(|i| input.is_char_boundary(*i))
                .collect();
            for &i in &bounds {
                let spoken = spoken_for(&[&input[..i], &input[i..]]);
                assert_eq!(norm(&spoken.join(" ")), expected, "split {i}: {input:?}");
                // Chunking never changes what is said, sentence for sentence.
                assert_eq!(spoken, whole, "split {i}: {input:?}");
            }
            for (n, &i) in bounds.iter().enumerate() {
                for &j in bounds.iter().skip(n + 1).step_by(7) {
                    let spoken = spoken_for(&[&input[..i], &input[i..j], &input[j..]]);
                    assert_eq!(spoken, whole, "split {i},{j}: {input:?}");
                }
            }
        }
    }

    #[test]
    fn tags_split_inside_their_names_still_speak_once() {
        let spoken = spoken_for(&["<T", "TS>Hello there, how are you?</TT", "S> shown"]);
        assert_eq!(spoken, vec!["Hello there, how are you?"]);
        let spoken = spoken_for(&["<TTS>Lower case close tag here.</t", "ts> not spoken"]);
        assert_eq!(spoken, vec!["Lower case close tag here."]);
    }

    #[test]
    fn a_nested_open_tag_is_not_read_aloud() {
        let (display, spoken) = split_tts_tags("<TTS>One <TTS>two.</TTS> three");
        assert_eq!(spoken, vec!["One two."]);
        assert_eq!(display, "three");
    }

    #[test]
    fn end_block_stops_an_unclosed_block_at_the_content_boundary() {
        let mut s = TtsTagStream::new();
        let (_, t1) = s.push("<TTS>Sure, opening it now");
        assert!(t1.is_empty());
        assert_eq!(s.end_block(), vec!["Sure, opening it now"]);
        // The next message's text is shown, never spoken.
        let (d2, t2) = s.push("Here is the list. It has three items.");
        assert_eq!(d2, "Here is the list. It has three items.");
        assert!(t2.is_empty());
        let (d3, t3) = s.finish();
        assert!(d3.is_empty() && t3.is_empty());
        // Nothing open: a no-op that keeps a held display fragment.
        let mut s = TtsTagStream::new();
        s.push("text <TT");
        assert!(s.end_block().is_empty());
        let (d, _) = s.push("S>spoken words here.</TTS>");
        assert_eq!(d, "");
    }

    #[test]
    fn a_half_arrived_close_tag_is_never_spoken() {
        let mut s = TtsTagStream::new();
        s.push("<TTS>Cut off right at the end.</TT");
        assert_eq!(s.end_block(), vec!["Cut off right at the end."]);
        let (_, spoken) = stream_all(&["<TTS>Ran out of tokens here.</T"]);
        assert_eq!(spoken, vec!["Ran out of tokens here."]);
    }

    #[test]
    fn unterminated_block_flushes_remaining_sentences_on_finish() {
        let (_, spoken) =
            stream_all(&["<TTS>Here is the first part. And the second that never clo"]);
        assert_eq!(
            spoken,
            vec!["Here is the first part.", "And the second that never clo"]
        );
    }
}
