//! The conversation starters on the empty state.
//!
//! Rust owns the list so every surface (main window, bar) shows the same ones
//! and React only renders. Each starter is something only an agent that can
//! use the computer can do, worded the way a person would say it, and each
//! names the tools it needs so a test can prove it still works today.

use serde::Serialize;

use crate::constants::agent::tool_names;

/// What the frontend gets: a short label and the words that get sent.
#[derive(Debug, Clone, Serialize)]
pub struct Starter {
    pub id: &'static str,
    pub title: &'static str,
    pub prompt: &'static str,
}

struct StarterSpec {
    starter: Starter,
    /// Tools the request needs. Every one must resolve to a registered family.
    tools: &'static [&'static str],
}

const SPECS: &[StarterSpec] = &[
    StarterSpec {
        starter: Starter {
            id: "screen",
            title: "What's on my screen?",
            prompt: "What's on my screen? Look at it and tell me what I'm working on.",
        },
        tools: &[tool_names::SCREENSHOT],
    },
    StarterSpec {
        starter: Starter {
            id: "chess",
            title: "Play chess with me",
            prompt: "Play chess with me. Open Chess and make the first move as white.",
        },
        tools: &[tool_names::OPEN_APPLICATION, tool_names::COMPUTER],
    },
    StarterSpec {
        starter: Starter {
            id: "downloads",
            title: "Tidy up my Downloads",
            prompt: "Tidy up my Downloads folder. Sort the files into folders by type, and move anything that looks like junk to the Trash.",
        },
        tools: &[tool_names::LIST_FILES, tool_names::BASH_COMMAND],
    },
    StarterSpec {
        starter: Starter {
            id: "flight",
            title: "Find me a flight",
            prompt: "Find me the cheapest flight to Tokyo next month and show me the best three options.",
        },
        tools: &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
        ],
    },
];

pub fn starters() -> Vec<Starter> {
    SPECS.iter().map(|spec| spec.starter.clone()).collect()
}

#[tauri::command]
pub fn get_starters() -> Vec<Starter> {
    starters()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::tool_mapping::ToolMappingService;
    use std::collections::HashSet;

    #[test]
    fn there_are_three_or_four_starters() {
        assert!((3..=4).contains(&starters().len()));
    }

    #[test]
    fn each_starter_maps_to_an_available_tool_family() {
        for spec in SPECS {
            assert!(!spec.tools.is_empty(), "{} names no tool", spec.starter.id);
            for tool in spec.tools {
                assert!(
                    ToolMappingService::get_tool_category(tool).is_some(),
                    "starter {:?} needs {:?}, which no tool family registers",
                    spec.starter.id,
                    tool
                );
            }
        }
    }

    #[test]
    fn starters_are_unique_and_plain() {
        let mut ids = HashSet::new();
        for spec in SPECS {
            let s = &spec.starter;
            assert!(ids.insert(s.id), "duplicate id {}", s.id);
            assert!(!s.title.is_empty() && !s.prompt.is_empty());
            assert!(!s.prompt.contains('\u{2014}') && !s.title.contains('\u{2014}'));
        }
    }
}
