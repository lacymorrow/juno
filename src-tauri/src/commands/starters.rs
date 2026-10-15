//! The conversation starters on the empty state.
//!
//! Rust owns the list so every surface (main window, bar) shows the same ones
//! and React only renders. Each starter is something only an agent that can
//! use the computer can do, worded the way a person would say it, and each
//! names the tools it needs so a test can prove it still works today.
//!
//! The starters grow with the person. A new conversation shows three: two at
//! the level their history earns and one a step past it, shuffled, and never
//! one they have already tapped. Someone on day one sees "What's on my
//! screen?"; someone with fifty conversations sees multi-app errands.

use std::collections::HashSet;

use serde::Serialize;
use tauri::AppHandle;

use crate::constants::agent::tool_names;
use crate::conversation_history::{self, TITLE_MAX_CHARS};

/// What the frontend gets: a short label and the words that get sent.
#[derive(Debug, Clone, Serialize)]
pub struct Starter {
    pub id: &'static str,
    pub title: &'static str,
    pub prompt: &'static str,
}

/// How far into Juno a starter reaches. Never shown to the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    /// One step, one tool, an answer in seconds.
    First,
    /// A few steps, usually in one app or the browser.
    Regular,
    /// An errand across apps that would take a person ten minutes.
    Seasoned,
}

impl Level {
    /// Conversations someone has actually had (a reply came back), to level.
    fn for_conversations(count: usize) -> Self {
        match count {
            0..=4 => Level::First,
            5..=19 => Level::Regular,
            _ => Level::Seasoned,
        }
    }

    fn next(self) -> Self {
        match self {
            Level::First => Level::Regular,
            Level::Regular | Level::Seasoned => Level::Seasoned,
        }
    }
}

/// How many starters a new conversation shows.
const SHOWN: usize = 3;

struct StarterSpec {
    starter: Starter,
    level: Level,
    /// Tools the request needs. Every one must resolve to a registered family.
    /// Read only by the test that holds each starter to a real tool.
    #[cfg_attr(not(test), allow(dead_code))]
    tools: &'static [&'static str],
}

const fn spec(
    level: Level,
    id: &'static str,
    title: &'static str,
    prompt: &'static str,
    tools: &'static [&'static str],
) -> StarterSpec {
    StarterSpec {
        starter: Starter { id, title, prompt },
        level,
        tools,
    }
}

const SPECS: &[StarterSpec] = &[
    // First: proves Juno can see and touch the Mac.
    spec(
        Level::First,
        "screen",
        "What's on my screen?",
        "What's on my screen? Look at it and tell me what I'm working on.",
        &[tool_names::SCREENSHOT],
    ),
    spec(
        Level::First,
        "chess",
        "Play chess with me",
        "Play chess with me. Open Chess and make the first move as white.",
        &[tool_names::OPEN_APPLICATION, tool_names::COMPUTER],
    ),
    spec(
        Level::First,
        "battery",
        "How's my battery?",
        "How's my battery doing, and how long until I need to plug in?",
        &[tool_names::GET_SYSTEM_INFO],
    ),
    spec(
        Level::First,
        "apps",
        "What apps are open?",
        "What apps do I have open right now?",
        &[tool_names::GET_RUNNING_APPLICATIONS],
    ),
    spec(
        Level::First,
        "weather",
        "What's the weather today?",
        "What's the weather where I am today? Do I need a jacket?",
        &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
        ],
    ),
    spec(
        Level::First,
        "dark-mode",
        "Turn on Dark Mode",
        "Switch my Mac to Dark Mode.",
        &[tool_names::BASH_COMMAND],
    ),
    // Regular: a few steps, one place.
    spec(
        Level::Regular,
        "downloads",
        "Tidy up my Downloads",
        "Tidy up my Downloads folder. Sort the files into folders by type, and move anything that looks like junk to the Trash.",
        &[tool_names::LIST_FILES, tool_names::BASH_COMMAND],
    ),
    spec(
        Level::Regular,
        "flight",
        "Find a cheap flight to Tokyo",
        "Find me the cheapest flight to Tokyo next month and show me the best three options.",
        &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
        ],
    ),
    spec(
        Level::Regular,
        "disk",
        "What's filling up my disk?",
        "Find the biggest files and folders in my home folder and tell me which are safe to delete.",
        &[tool_names::BASH_COMMAND],
    ),
    spec(
        Level::Regular,
        "page",
        "Sum up this web page",
        "Read the page I have open in my browser and give me the three things worth knowing.",
        &[
            tool_names::BROWSER_GET_CURRENT_URL,
            tool_names::BROWSER_EXTRACT_CONTENT,
        ],
    ),
    spec(
        Level::Regular,
        "news",
        "Catch me up on tech news",
        "Catch me up on today's top tech news in five short bullet points.",
        &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
        ],
    ),
    spec(
        Level::Regular,
        "focus",
        "Help me focus",
        "Quit every app except the one I'm using so I can focus.",
        &[
            tool_names::GET_RUNNING_APPLICATIONS,
            tool_names::QUIT_APPLICATION,
        ],
    ),
    spec(
        Level::Regular,
        "backup",
        "Back up my Documents",
        "Make a dated zip backup of my Documents folder and put it on my Desktop.",
        &[tool_names::BASH_COMMAND],
    ),
    // Seasoned: errands that cross apps.
    spec(
        Level::Seasoned,
        "laptops",
        "Compare laptops in a table",
        "Compare the MacBook Air, MacBook Pro, and Dell XPS 13 on price, battery life, and weight, and put it in a table in Numbers.",
        &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
            tool_names::OPEN_APPLICATION,
            tool_names::COMPUTER,
        ],
    ),
    spec(
        Level::Seasoned,
        "trip",
        "Plan a weekend away",
        "Plan a weekend in Asheville: two places to stay under $200 a night and three things to do. Save it all to a new note.",
        &[
            tool_names::BROWSER_NAVIGATE,
            tool_names::BROWSER_EXTRACT_CONTENT,
            tool_names::OPEN_APPLICATION,
            tool_names::COMPUTER,
        ],
    ),
    spec(
        Level::Seasoned,
        "receipts",
        "Add up this month's receipts",
        "Find the PDF receipts in my Downloads from this month and add up what I spent.",
        &[tool_names::LIST_FILES, tool_names::BASH_COMMAND],
    ),
    spec(
        Level::Seasoned,
        "workspace",
        "Set up my workspace",
        "Open Mail, Calendar, and Notes and arrange them side by side on my screen.",
        &[tool_names::OPEN_APPLICATION, tool_names::COMPUTER],
    ),
    spec(
        Level::Seasoned,
        "code-today",
        "What did I code today?",
        "Look through my projects for git repos I committed to today and write me a short summary of what changed.",
        &[tool_names::BASH_COMMAND],
    ),
    spec(
        Level::Seasoned,
        "slides",
        "Turn a note into slides",
        "Take my most recent note in Notes and turn it into a short Keynote presentation.",
        &[
            tool_names::OPEN_APPLICATION,
            tool_names::ACCESSIBILITY_SCAN,
            tool_names::COMPUTER,
        ],
    ),
    spec(
        Level::Seasoned,
        "calendar",
        "Find time for a call this week",
        "Look at my calendar and find three open hours this week that would work for a one hour call.",
        &[tool_names::OPEN_APPLICATION, tool_names::SCREENSHOT],
    ),
];

/// A saved conversation's title is its first query cut to the title length,
/// so a starter that opened one has a title equal to its prompt, cut the same.
fn opened_by(spec: &StarterSpec, titles: &HashSet<String>) -> bool {
    let cut: String = spec.starter.prompt.chars().take(TITLE_MAX_CHARS).collect();
    titles.contains(&cut)
}

/// Deterministic shuffle key so `pick` is pure and testable.
fn shuffle_key(seed: u64, id: &str) -> u64 {
    // FNV-1a over the id, mixed with the seed.
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for b in id.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Three starters for someone with `conversations` real conversations behind
/// them, none of whose titles they have already sent.
///
/// Two come from their level and one from the next (three Seasoned at the
/// top). A level that runs dry borrows from the nearest one, untried first; if
/// they have tried everything, everything is fair game again.
fn pick(conversations: usize, tried_titles: &HashSet<String>, seed: u64) -> Vec<Starter> {
    let level = Level::for_conversations(conversations);
    let stretch = level.next();

    let mut ranked: Vec<&StarterSpec> = SPECS.iter().collect();
    ranked.sort_by_key(|s| {
        let tried = opened_by(s, tried_titles);
        // Distance from where they are: their level, then the stretch, then
        // the rest, nearer first.
        let distance = (s.level as i32 - level as i32).unsigned_abs();
        (tried, distance, shuffle_key(seed, s.starter.id))
    });

    let mut chosen: Vec<&StarterSpec> = Vec::with_capacity(SHOWN);
    // One stretch slot, untried, when it differs from their level.
    if stretch != level {
        if let Some(s) = ranked
            .iter()
            .find(|s| s.level == stretch && !opened_by(s, tried_titles))
        {
            chosen.push(s);
        }
    }
    for s in &ranked {
        if chosen.len() == SHOWN {
            break;
        }
        if !chosen.iter().any(|c| c.starter.id == s.starter.id) {
            chosen.push(s);
        }
    }

    // Easier first, so the row reads as a path, shuffled within a level.
    chosen.sort_by_key(|s| (s.level, shuffle_key(seed, s.starter.id)));
    chosen.into_iter().map(|s| s.starter.clone()).collect()
}

#[tauri::command]
pub fn get_starters(app: AppHandle) -> Vec<Starter> {
    let history = conversation_history::list(&app);
    // A conversation counts once a reply came back.
    let conversations = history.iter().filter(|m| m.message_count >= 2).count();
    let titles: HashSet<String> = history.into_iter().map(|m| m.title).collect();
    let seed = uuid::Uuid::new_v4().as_u64_pair().0;
    pick(conversations, &titles, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::tool_mapping::ToolMappingService;

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    fn level_of(id: &str) -> Level {
        SPECS.iter().find(|s| s.starter.id == id).unwrap().level
    }

    #[test]
    fn always_three_distinct_starters() {
        for conversations in [0, 3, 5, 12, 20, 500] {
            for seed in 0..50 {
                let got = pick(conversations, &none(), seed);
                assert_eq!(got.len(), SHOWN);
                let ids: HashSet<_> = got.iter().map(|s| s.id).collect();
                assert_eq!(ids.len(), SHOWN, "duplicate starter");
            }
        }
    }

    #[test]
    fn day_one_is_two_first_and_one_regular() {
        for seed in 0..50 {
            let levels: Vec<Level> = pick(0, &none(), seed)
                .iter()
                .map(|s| level_of(s.id))
                .collect();
            assert_eq!(levels, vec![Level::First, Level::First, Level::Regular]);
        }
    }

    #[test]
    fn regulars_get_one_seasoned_stretch() {
        let levels: Vec<Level> = pick(10, &none(), 7)
            .iter()
            .map(|s| level_of(s.id))
            .collect();
        assert_eq!(
            levels,
            vec![Level::Regular, Level::Regular, Level::Seasoned]
        );
    }

    #[test]
    fn seasoned_see_only_seasoned() {
        for seed in 0..50 {
            assert!(pick(100, &none(), seed)
                .iter()
                .all(|s| level_of(s.id) == Level::Seasoned));
        }
    }

    #[test]
    fn the_row_changes_between_conversations() {
        let rows: HashSet<Vec<&str>> = (0..20)
            .map(|seed| pick(0, &none(), seed).iter().map(|s| s.id).collect())
            .collect();
        assert!(rows.len() > 1, "every seed gave the same row");
    }

    #[test]
    fn a_tapped_starter_does_not_come_back() {
        let screen = SPECS.iter().find(|s| s.starter.id == "screen").unwrap();
        let title: String = screen
            .starter
            .prompt
            .chars()
            .take(TITLE_MAX_CHARS)
            .collect();
        let tried = HashSet::from([title]);
        for seed in 0..50 {
            assert!(pick(0, &tried, seed).iter().all(|s| s.id != "screen"));
        }
    }

    #[test]
    fn having_tried_everything_still_shows_three() {
        let all: HashSet<String> = SPECS
            .iter()
            .map(|s| s.starter.prompt.chars().take(TITLE_MAX_CHARS).collect())
            .collect();
        assert_eq!(pick(0, &all, 1).len(), SHOWN);
        assert_eq!(pick(100, &all, 1).len(), SHOWN);
    }

    #[test]
    fn every_level_can_fill_a_row_alone() {
        for level in [Level::First, Level::Regular, Level::Seasoned] {
            assert!(SPECS.iter().filter(|s| s.level == level).count() >= SHOWN + 2);
        }
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
        let mut cuts = HashSet::new();
        for spec in SPECS {
            let s = &spec.starter;
            assert!(ids.insert(s.id), "duplicate id {}", s.id);
            // Prompts must differ within the title length, or tapping one
            // would hide the other.
            let cut: String = s.prompt.chars().take(TITLE_MAX_CHARS).collect();
            assert!(cuts.insert(cut), "{} shares a title with another", s.id);
            assert!(!s.title.is_empty() && !s.prompt.is_empty());
            assert!(!s.prompt.contains('\u{2014}') && !s.title.contains('\u{2014}'));
        }
    }
}
