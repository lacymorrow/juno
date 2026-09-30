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

/// Incremental `<TTS>` parser for streamed text.
///
/// `push` returns the display text that is safe to show now plus every
/// complete spoken block that closed inside this chunk. Characters that could
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
                if rest.starts_with(CLOSE_TAG) {
                    if !self.spoken.trim().is_empty() {
                        spoken_blocks.push(std::mem::take(&mut self.spoken));
                    } else {
                        self.spoken.clear();
                    }
                    self.in_tag = false;
                    consumed += CLOSE_TAG.len();
                    continue;
                }
                if CLOSE_TAG.starts_with(rest) {
                    // Possible partial "</TTS>" at the end; wait for more.
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
        (display, spoken_blocks)
    }

    /// Flush at end of stream. Returns `(display_text, spoken_blocks)`.
    ///
    /// A block that never closed is still spoken (the model ran out of tokens
    /// or the tag was malformed); a partial tag left in the buffer is shown
    /// as plain text so nothing is silently dropped.
    pub fn finish(&mut self) -> (String, Vec<String>) {
        let mut display = String::new();
        let mut spoken_blocks = Vec::new();

        let tail = std::mem::take(&mut self.buffer);
        if self.in_tag {
            self.spoken.push_str(&tail);
            if !self.spoken.trim().is_empty() {
                spoken_blocks.push(std::mem::take(&mut self.spoken));
            }
            self.spoken.clear();
            self.in_tag = false;
        } else {
            display.push_str(&tail);
        }

        (display, spoken_blocks)
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
}
