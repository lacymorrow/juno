//! Skill discovery for slash-command autocomplete (LAC-3031).
//!
//! Enumerates the user's Claude Code skills and custom slash commands so the
//! frontend can offer fuzzy autocomplete when the user types `/...` in an
//! input. Filesystem enumeration lives here (backend owns I/O); the frontend
//! only renders the returned list.

use serde::Serialize;
use std::fs;
use std::path::Path;

/// Maximum number of characters of a description surfaced to the UI.
const MAX_DESCRIPTION_CHARS: usize = 120;
/// Safety cap so a pathological ~/.claude directory cannot flood the UI.
const MAX_SKILLS: usize = 500;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SkillInfo {
    /// Slash-command name without the leading slash, e.g. "paperclip".
    pub name: String,
    /// One-line human description (may be empty when none is declared).
    pub description: String,
    /// Where this entry came from: "skill" or "command".
    pub source: String,
}

/// List the user's available skills and custom slash commands.
///
/// Scans `~/.claude/skills/*/SKILL.md` and `~/.claude/commands/**/*.md`.
/// Returns entries sorted by name, deduplicated (skills win over commands).
#[tauri::command]
pub async fn list_available_skills() -> Result<Vec<SkillInfo>, String> {
    tokio::task::spawn_blocking(collect_skills)
        .await
        .map_err(|e| format!("Skill discovery task failed: {}", e))
}

fn collect_skills() -> Vec<SkillInfo> {
    let mut results: Vec<SkillInfo> = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let claude_dir = home.join(".claude");
        collect_skill_dirs(&claude_dir.join("skills"), &mut results);
        collect_command_files(&claude_dir.join("commands"), "", 0, &mut results);
    }

    // Sort by name; keep "skill" entries ahead of "command" entries with the
    // same name so dedup_by_key keeps the richer skill metadata.
    results.sort_by(|a, b| a.name.cmp(&b.name).then(a.source.cmp(&b.source)));
    results.dedup_by(|a, b| a.name == b.name);
    results.truncate(MAX_SKILLS);
    results
}

/// Each subdirectory of `skills/` containing a SKILL.md is one skill.
fn collect_skill_dirs(skills_dir: &Path, out: &mut Vec<SkillInfo>) {
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("SKILL.md");
        if !manifest.is_file() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let content = fs::read_to_string(&manifest).unwrap_or_default();
        let front = parse_frontmatter(&content);
        let name = front
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| dir_name.to_string());
        out.push(SkillInfo {
            name,
            description: truncate_chars(&front.description.unwrap_or_default()),
            source: "skill".to_string(),
        });
    }
}

/// Each `.md` file under `commands/` is a custom slash command. One level of
/// subdirectory namespacing is supported, matching Claude Code's
/// `/<dir>:<name>` convention.
fn collect_command_files(dir: &Path, prefix: &str, depth: u8, out: &mut Vec<SkillInfo>) {
    if depth > 1 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            let nested_prefix = format!("{}{}:", prefix, stem);
            collect_command_files(&path, &nested_prefix, depth + 1, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        let front = parse_frontmatter(&content);
        let description = front
            .description
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| first_content_line(&content));
        out.push(SkillInfo {
            name: format!("{}{}", prefix, stem),
            description: truncate_chars(&description),
            source: "command".to_string(),
        });
    }
}

struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

/// Minimal YAML frontmatter reader: extracts top-level `name:` and
/// `description:` scalar values from a leading `---` block. Intentionally not
/// a full YAML parser — skill manifests only need these two keys here.
fn parse_frontmatter(content: &str) -> Frontmatter {
    let mut front = Frontmatter {
        name: None,
        description: None,
    };
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return front;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        // Only top-level keys (no leading indentation on the raw line).
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("name:") {
            front.name = Some(clean_yaml_scalar(value));
        } else if let Some(value) = trimmed.strip_prefix("description:") {
            front.description = Some(clean_yaml_scalar(value));
        }
    }
    front
}

fn clean_yaml_scalar(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })
        .unwrap_or(trimmed);
    unquoted.trim().to_string()
}

/// First non-empty markdown line outside frontmatter, stripped of heading
/// markers — used as a fallback description for command files.
fn first_content_line(content: &str) -> String {
    let mut in_frontmatter = false;
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        return trimmed.trim_start_matches('#').trim().to_string();
    }
    String::new()
}

/// Char-based truncation (byte slicing panics on multi-byte UTF-8).
fn truncate_chars(text: &str) -> String {
    if text.chars().count() <= MAX_DESCRIPTION_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_DESCRIPTION_CHARS).collect();
    format!("{}…", truncated.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_name_and_description() {
        let content = "---\nname: paperclip\ndescription: \"Interact with the Paperclip API\"\nother: x\n---\n# Body\n";
        let front = parse_frontmatter(content);
        assert_eq!(front.name.as_deref(), Some("paperclip"));
        assert_eq!(
            front.description.as_deref(),
            Some("Interact with the Paperclip API")
        );
    }

    #[test]
    fn missing_frontmatter_returns_none() {
        let front = parse_frontmatter("# Just a heading\nbody text\n");
        assert!(front.name.is_none());
        assert!(front.description.is_none());
    }

    #[test]
    fn nested_frontmatter_keys_are_ignored() {
        let content = "---\nmetadata:\n  name: nested\ndescription: top level\n---\n";
        let front = parse_frontmatter(content);
        assert!(front.name.is_none());
        assert_eq!(front.description.as_deref(), Some("top level"));
    }

    #[test]
    fn first_content_line_skips_frontmatter_and_headings() {
        let content = "---\nallowed-tools: Bash\n---\n\n# Review the PR\nDo the thing.\n";
        assert_eq!(first_content_line(content), "Review the PR");
    }

    #[test]
    fn truncate_handles_multibyte_utf8() {
        let long: String = "é".repeat(MAX_DESCRIPTION_CHARS + 10);
        let out = truncate_chars(&long);
        assert!(out.chars().count() <= MAX_DESCRIPTION_CHARS + 1);
        assert!(out.ends_with('…'));
    }
}
