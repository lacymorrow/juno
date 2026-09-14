//! A response as a document.
//!
//! Copy, the share sheet and "Save as…" all start from a `ResponseExport`: the
//! question that was asked, when, and the three channels a Juno reply is made
//! of: what was **spoken** (the `<TTS>` channel), what was **written** (the
//! prose the person read) and what was **generated** (the components the
//! agent rendered). The plain-text form is what lands on the clipboard and in
//! Messages or Mail; the Markdown and HTML forms are the saved documents, laid
//! out the same way every time so a Juno export is recognisable as one.

use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;

/// What the chat surface knows about one assistant message.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResponseExportInput {
    /// The user message this reply answers, when there is one.
    #[serde(default)]
    pub question: Option<String>,
    /// The stored message content, `<TTS>` markers and components included.
    pub content: String,
    /// Spoken parts the streaming pipeline already lifted out of `content`.
    #[serde(default)]
    pub spoken: Vec<String>,
    /// Unix milliseconds of the message.
    #[serde(default)]
    pub timestamp: Option<i64>,
}

/// One component the agent rendered inline, kept as its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedBlock {
    pub component: String,
    pub source: String,
}

impl GeneratedBlock {
    /// The words a person could read inside the component: tags and JSX
    /// expressions removed, one line per source line (the agent writes one
    /// element per line, so inline spans stay together). Attribute values are
    /// not text, so a card's title comes through only if it was a child.
    pub fn text(&self) -> String {
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut depth_tag = false;
        let mut depth_expr = 0usize;
        for c in self.source.chars() {
            match c {
                '<' if depth_expr == 0 => {
                    depth_tag = true;
                    current.push(' ');
                }
                '>' if depth_tag => depth_tag = false,
                '{' if !depth_tag => depth_expr += 1,
                '}' if !depth_tag && depth_expr > 0 => depth_expr -= 1,
                '\n' if !depth_tag && depth_expr == 0 => push_line(&mut lines, &mut current),
                _ if !depth_tag && depth_expr == 0 => current.push(c),
                _ => {}
            }
        }
        push_line(&mut lines, &mut current);
        lines.join("\n")
    }
}

fn push_line(lines: &mut Vec<String>, current: &mut String) {
    let line = current.split_whitespace().collect::<Vec<_>>().join(" ");
    if !line.is_empty() {
        lines.push(line);
    }
    current.clear();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseExport {
    pub question: Option<String>,
    pub asked_at: Option<DateTime<Local>>,
    pub spoken: Vec<String>,
    pub written: String,
    pub generated: Vec<GeneratedBlock>,
}

/// File formats a response can be saved as from the share sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Html,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "Markdown",
            ExportFormat::Html => "HTML",
        }
    }
}

const APP_NAME: &str = "Juno";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_TITLE_CHARS: usize = 60;
const FALLBACK_TITLE: &str = "Juno response";
const TTS_OPEN: &str = "<TTS>";
const TTS_CLOSE: &str = "</TTS>";

impl ResponseExport {
    pub fn from_input(input: ResponseExportInput) -> Self {
        let (mut spoken, rest) = split_spoken(&input.content);
        for part in input.spoken {
            let part = part.trim().to_string();
            if !part.is_empty() && !spoken.contains(&part) {
                spoken.push(part);
            }
        }
        let (generated, written) = split_generated(&rest);
        let asked_at = input
            .timestamp
            .and_then(|ms| Local.timestamp_millis_opt(ms).single());
        Self {
            question: input
                .question
                .map(|q| q.trim().to_string())
                .filter(|q| !q.is_empty()),
            asked_at,
            spoken,
            written: collapse_blank_lines(written.trim()),
            generated,
        }
    }

    /// The document's name: the question, tidied into a title, or the first
    /// line of the reply when there was no question.
    pub fn title(&self) -> String {
        let source = self
            .question
            .as_deref()
            .and_then(first_line)
            .or_else(|| first_line(&self.written))
            .or_else(|| self.spoken.first().map(|s| s.as_str()));
        match source {
            Some(line) => title_case_start(&clip_words(
                strip_markdown_marks(line)
                    .trim_end_matches(['.', '!', ':', ';', ','])
                    .trim(),
                MAX_TITLE_CHARS,
            )),
            None => FALLBACK_TITLE.to_string(),
        }
    }

    /// A file name for the save panel: the title with characters a file
    /// system rejects turned into spaces.
    pub fn file_name(&self, format: ExportFormat) -> String {
        let title = self.title();
        let cleaned: String = title
            .chars()
            .filter(|c| !matches!(c, '\'' | '\u{2019}' | ','))
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let stem = if cleaned.is_empty() {
            FALLBACK_TITLE.to_string()
        } else {
            cleaned
        };
        format!("{stem}.{}", format.extension())
    }

    /// The reply as the person read it: spoken paragraphs, the prose, then
    /// the words inside each generated component.
    pub fn plain_text(&self) -> String {
        let mut parts: Vec<String> = self.spoken.clone();
        if !self.written.is_empty() {
            parts.push(self.written.clone());
        }
        parts.extend(
            self.generated
                .iter()
                .map(GeneratedBlock::text)
                .filter(|t| !t.is_empty()),
        );
        parts.join("\n\n")
    }

    pub fn render(&self, format: ExportFormat) -> String {
        match format {
            ExportFormat::Markdown => self.markdown(),
            ExportFormat::Html => self.html(),
        }
    }

    pub fn markdown(&self) -> String {
        let title = self.title();
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("title: \"{}\"\n", title.replace('"', "\\\"")));
        if let Some(at) = self.asked_at {
            out.push_str(&format!("date: {}\n", at.to_rfc3339()));
        }
        out.push_str(&format!("app: {APP_NAME} {APP_VERSION}\n"));
        out.push_str(&format!("spoken: {}\n", self.spoken.len()));
        out.push_str(&format!(
            "generated: [{}]\n",
            self.generated
                .iter()
                .map(|g| g.component.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str("---\n\n");
        out.push_str(&format!("# {title}\n\n"));

        if let Some(question) = &self.question {
            out.push_str("## Asked\n\n");
            for line in question.lines() {
                out.push_str(&format!("> {line}\n"));
            }
            out.push('\n');
        }
        if !self.spoken.is_empty() {
            out.push_str("## Spoken\n\n");
            out.push_str(&self.spoken.join("\n\n"));
            out.push_str("\n\n");
        }
        if !self.written.is_empty() {
            out.push_str("## Written\n\n");
            out.push_str(&self.written);
            out.push_str("\n\n");
        }
        if !self.generated.is_empty() {
            out.push_str("## Generated\n\n");
            for block in &self.generated {
                out.push_str(&format!("`{}`\n\n", block.component));
                let text = block.text();
                if !text.is_empty() {
                    out.push_str(&text);
                    out.push_str("\n\n");
                }
                out.push_str(&format!("```jsx\n{}\n```\n\n", block.source));
            }
        }
        out.push_str("---\n\n");
        out.push_str(&self.footer_line());
        out.push('\n');
        out
    }

    pub fn html(&self) -> String {
        let esc = html_escape::encode_text;
        let title = self.title();
        let mut body = String::new();
        body.push_str(&format!("    <h1>{}</h1>\n", esc(&title)));
        if let Some(question) = &self.question {
            body.push_str("    <section>\n      <h2>Asked</h2>\n");
            body.push_str(&format!(
                "      <blockquote>{}</blockquote>\n",
                esc(question)
            ));
            body.push_str("    </section>\n");
        }
        if !self.spoken.is_empty() {
            body.push_str("    <section>\n      <h2>Spoken</h2>\n");
            for part in &self.spoken {
                body.push_str(&format!("      <p class=\"spoken\">{}</p>\n", esc(part)));
            }
            body.push_str("    </section>\n");
        }
        if !self.written.is_empty() {
            body.push_str("    <section>\n      <h2>Written</h2>\n");
            body.push_str(&format!(
                "      <div class=\"written\">{}</div>\n",
                esc(&self.written)
            ));
            body.push_str("    </section>\n");
        }
        if !self.generated.is_empty() {
            body.push_str("    <section>\n      <h2>Generated</h2>\n");
            for block in &self.generated {
                body.push_str(&format!(
                    "      <p class=\"component\">{}</p>\n",
                    esc(&block.component)
                ));
                let text = block.text();
                if !text.is_empty() {
                    body.push_str(&format!(
                        "      <div class=\"written\">{}</div>\n",
                        esc(&text)
                    ));
                }
                body.push_str(&format!(
                    "      <pre><code>{}</code></pre>\n",
                    esc(&block.source)
                ));
            }
            body.push_str("    </section>\n");
        }
        body.push_str(&format!(
            "    <footer>{}</footer>\n",
            esc(&self.footer_line())
        ));

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        :root {{ color-scheme: light dark; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
            font-size: 17px;
            line-height: 1.5;
            max-width: 40em;
            margin: 0 auto;
            padding: 64px 24px 48px;
            color: #1d1d1f;
            background: #fff;
        }}
        h1 {{ font-size: 32px; font-weight: 700; letter-spacing: -0.02em; line-height: 1.15; margin: 0 0 40px; text-wrap: balance; }}
        h2 {{ font-size: 12px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: #86868b; margin: 32px 0 10px; }}
        blockquote {{ margin: 0; padding: 0 0 0 16px; border-left: 3px solid #d2d2d7; color: #515154; white-space: pre-wrap; }}
        .spoken {{ font-style: italic; margin: 0 0 12px; }}
        .written {{ white-space: pre-wrap; word-wrap: break-word; }}
        .component {{ font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 14px; color: #515154; margin: 0 0 6px; }}
        pre {{ background: #f5f5f7; border-radius: 10px; padding: 14px 16px; overflow-x: auto; font-size: 13px; }}
        footer {{ margin-top: 56px; padding-top: 16px; border-top: 1px solid #d2d2d7; font-size: 13px; color: #86868b; }}
        @media (prefers-color-scheme: dark) {{
            body {{ color: #f5f5f7; background: #1d1d1f; }}
            blockquote {{ border-color: #424245; color: #a1a1a6; }}
            pre {{ background: #2c2c2e; }}
            footer {{ border-color: #424245; }}
        }}
    </style>
</head>
<body>
{body}</body>
</html>
"#,
            title = esc(&title),
            body = body
        )
    }

    fn footer_line(&self) -> String {
        match self.asked_at {
            Some(at) => format!(
                "{APP_NAME} {APP_VERSION} · {}",
                at.format("%B %-d, %Y at %-I:%M %p")
            ),
            None => format!("{APP_NAME} {APP_VERSION}"),
        }
    }
}

/// Lift every `<TTS>…</TTS>` block out of `content`. Returns the spoken parts
/// and the content with the blocks removed. A stray marker without its pair
/// is dropped on its own.
fn split_spoken(content: &str) -> (Vec<String>, String) {
    let mut spoken = Vec::new();
    let mut rest = String::with_capacity(content.len());
    let mut cursor = content;
    while let Some(open) = cursor.find(TTS_OPEN) {
        rest.push_str(&cursor[..open]);
        let after_open = &cursor[open + TTS_OPEN.len()..];
        match after_open.find(TTS_CLOSE) {
            Some(close) => {
                let part = after_open[..close].trim();
                if !part.is_empty() {
                    spoken.push(part.to_string());
                }
                cursor = &after_open[close + TTS_CLOSE.len()..];
            }
            None => {
                cursor = after_open;
            }
        }
    }
    rest.push_str(cursor);
    (spoken, rest.replace(TTS_CLOSE, ""))
}

/// Lift every rendered component (`<Name …>…</Name>` or `<Name … />`) out of
/// the prose. Prose that merely looks like a tag (`Vec<String>`) is left
/// alone: a component tag is `<`, an uppercase letter, and a name ended by
/// whitespace, `>` or `/`, exactly what the renderer treats as one.
fn split_generated(content: &str) -> (Vec<GeneratedBlock>, String) {
    let mut generated = Vec::new();
    let mut written = String::with_capacity(content.len());
    let mut cursor = content;
    while let Some((start, name)) = find_component_open(cursor) {
        let Some(end) = component_end(cursor, start, &name) else {
            // No close in sight: it is prose after all.
            written.push_str(&cursor[..start + 1]);
            cursor = &cursor[start + 1..];
            continue;
        };
        written.push_str(&cursor[..start]);
        generated.push(GeneratedBlock {
            component: name,
            source: cursor[start..end].trim().to_string(),
        });
        cursor = &cursor[end..];
    }
    written.push_str(cursor);
    (generated, written)
}

fn find_component_open(text: &str) -> Option<(usize, String)> {
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find('<') {
        let start = search_from + rel;
        let after = &text[start + 1..];
        let name_len = after
            .char_indices()
            .take_while(|(i, c)| {
                (*i == 0 && c.is_ascii_uppercase()) || (*i > 0 && c.is_ascii_alphanumeric())
            })
            .count();
        if name_len > 0 {
            let name = &after[..name_len];
            let terminated = after[name_len..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/');
            if terminated && name != "TTS" {
                return Some((start, name.to_string()));
            }
        }
        search_from = start + 1;
    }
    None
}

/// Byte offset just past the component that opens at `start`.
fn component_end(text: &str, start: usize, name: &str) -> Option<usize> {
    let after = &text[start..];
    let close_tag = format!("</{name}>");
    if let Some(close) = after.find(&close_tag) {
        return Some(start + close + close_tag.len());
    }
    let open_end = after.find('>')?;
    if after[..open_end].ends_with('/') {
        return Some(start + open_end + 1);
    }
    None
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

fn strip_markdown_marks(line: &str) -> String {
    line.trim_start_matches(['#', '*', '-', '>', ' '])
        .replace("**", "")
        .replace('`', "")
}

/// Cut on a word boundary at `max` characters, adding an ellipsis.
fn clip_words(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(max).collect();
    if let Some(space) = clipped.rfind(' ') {
        clipped.truncate(space);
    }
    format!("{}…", clipped.trim_end_matches(['.', ',', ';', ':']))
}

fn title_case_start(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(question: Option<&str>, content: &str) -> ResponseExport {
        ResponseExport::from_input(ResponseExportInput {
            question: question.map(str::to_string),
            content: content.to_string(),
            spoken: vec![],
            timestamp: None,
        })
    }

    #[test]
    fn splits_spoken_written_and_generated() {
        let e = export(
            Some("what's playing?"),
            "<TTS>Here you go.</TTS>\n\nNow playing on Spotify.\n\n<NowPlayingCard title=\"Song\" />\n",
        );
        assert_eq!(e.spoken, vec!["Here you go."]);
        assert_eq!(e.written, "Now playing on Spotify.");
        assert_eq!(e.generated.len(), 1);
        assert_eq!(e.generated[0].component, "NowPlayingCard");
        assert_eq!(e.generated[0].source, "<NowPlayingCard title=\"Song\" />");
    }

    #[test]
    fn keeps_nested_component_source_whole() {
        let e = export(None, "Intro\n<Card title=\"x\"><Row>a</Row></Card>\nOutro");
        assert_eq!(
            e.generated[0].source,
            "<Card title=\"x\"><Row>a</Row></Card>"
        );
        assert_eq!(e.written, "Intro\n\nOutro");
    }

    #[test]
    fn leaves_generic_looking_prose_alone() {
        let e = export(None, "Use Vec<String> here, and a <b>bold</b> tag.");
        assert!(e.generated.is_empty());
        assert_eq!(e.written, "Use Vec<String> here, and a <b>bold</b> tag.");
    }

    #[test]
    fn title_comes_from_the_question() {
        let e = export(
            Some("Reply with exactly three short bullet points about the moon. No preamble."),
            "- a\n- b",
        );
        assert_eq!(
            e.title(),
            "Reply with exactly three short bullet points about the…"
        );
        assert_eq!(
            e.file_name(ExportFormat::Markdown),
            "Reply with exactly three short bullet points about the.md"
        );
        assert_eq!(
            export(Some("what's playing?"), "x").title(),
            "What's playing?"
        );
        assert_eq!(
            export(None, "## Weather in Charlotte\n\nSunny.").title(),
            "Weather in Charlotte"
        );
        assert_eq!(export(None, "").title(), "Juno response");
    }

    #[test]
    fn plain_text_is_what_the_person_read() {
        let e = export(None, "<TTS>Spoken.</TTS>\n\nWritten.\n\n<Card />");
        assert_eq!(e.plain_text(), "Spoken.\n\nWritten.");
        let e = export(
            None,
            "<TTS>Here.</TTS>\n<AnimatedCard animation=\"fade-up\">\n  <h3 className=\"x\">Moon</h3>\n  <AnimatedList gap={2}>\n    <div>🪨 <span className=\"b\">Crust:</span> rock.</div>\n    <div>Core: iron {1 + 1}.</div>\n  </AnimatedList>\n</AnimatedCard>",
        );
        assert_eq!(e.written, "");
        assert_eq!(e.generated[0].text(), "Moon\n🪨 Crust: rock.\nCore: iron .");
        assert_eq!(
            e.plain_text(),
            "Here.\n\nMoon\n🪨 Crust: rock.\nCore: iron ."
        );
    }

    #[test]
    fn markdown_has_front_matter_and_only_the_sections_present() {
        let md = export(Some("Q?"), "<TTS>S.</TTS>\n\nW.").markdown();
        assert!(md.starts_with("---\ntitle: \"Q?\"\n"));
        assert!(md.contains("spoken: 1\ngenerated: []\n"));
        assert!(
            md.contains("# Q?\n\n## Asked\n\n> Q?\n\n## Spoken\n\nS.\n\n## Written\n\nW.\n\n---")
        );
        assert!(!md.contains("## Generated"));
        assert!(md.trim_end().ends_with(&format!("Juno {APP_VERSION}")));
    }

    #[test]
    fn html_escapes_and_carries_the_sections() {
        let html = export(Some("a < b"), "c & d").html();
        assert!(html.contains("<blockquote>a &lt; b</blockquote>"));
        assert!(html.contains("<div class=\"written\">c &amp; d</div>"));
        assert!(html.contains("<title>A &lt; b</title>"));
    }
}
