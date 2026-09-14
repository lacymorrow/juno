//! Shared `<TTS>` tag extraction for every AI provider.
//!
//! The system prompt asks the model to put the spoken channel inside
//! `<TTS>...</TTS>` and everything else outside it. Each provider hands its
//! text through here so the spoken text reaches the speaker and the display
//! text reaches the chat with the tags removed. Keeping the parser in one place
//! means a provider cannot forget to speak (LAC: the Claude CLI provider never
//! extracted the tags, so nothing was spoken and raw tags showed in the chat).
//!
//! Two entry points:
//! - [`TtsTagStream`] for streaming providers. Feed chunks as they arrive; tags
//!   split across chunk boundaries are held back until they resolve.
//! - [`split_tts_tags`] for whole strings (non-streaming providers and the
//!   runner's final safety net).

const OPEN_TAG: &str = "<TTS>";
const CLOSE_TAG: &str = "</TTS>";

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
}
