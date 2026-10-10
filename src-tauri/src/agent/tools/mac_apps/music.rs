//! Play something from the person's Apple Music library by name.
//!
//! Music's own scripting search finds the match and plays it; the query
//! travels as `argv`, never as script text. Spotify has no scripting search,
//! so when Spotify is the app in front the answer says so and nothing is
//! guessed (`docs/plans/siri-parity.md`, "Considered and cut").
//!
//! The first call scripts Music, so macOS asks once for permission to control
//! it. Declined is one sentence and the one action that changes it.

use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::proc;
use super::{required_text, text_arg};
use crate::constants::permissions::urls;

/// The Automation dialog waits on a person, so the search may take a while.
const SEARCH_WAIT: Duration = Duration::from_secs(120);
const READ_WAIT: Duration = Duration::from_secs(10);

/// Separates the fields a script hands back.
const SEP: char = '\u{1f}';

/// What the person asked for, narrowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Song,
    Album,
    Artist,
    Playlist,
    /// No kind given: search everything in the library.
    Any,
}

impl Kind {
    pub fn parse(text: Option<&str>) -> Result<Kind, String> {
        match text.map(|t| t.trim().to_lowercase()).as_deref() {
            None => Ok(Kind::Any),
            Some("song" | "track") => Ok(Kind::Song),
            Some("album") => Ok(Kind::Album),
            Some("artist" | "band") => Ok(Kind::Artist),
            Some("playlist") => Ok(Kind::Playlist),
            Some(other) => Err(format!(
                "{other:?} is not a kind of music. Use song, album, artist or playlist."
            )),
        }
    }

    /// The word the script branches on.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Song => "song",
            Kind::Album => "album",
            Kind::Artist => "artist",
            Kind::Playlist => "playlist",
            Kind::Any => "any",
        }
    }
}

/// Search the library and play the first match. Constant text: `argv` item 1
/// is the query, item 2 the kind word. Answers `ok`, `none`, or `error` and
/// the AppleScript error number, so nothing here reads an error's wording.
const SEARCH_AND_PLAY: &str = r#"on run argv
  set theQuery to item 1 of argv
  set theKind to item 2 of argv
  set sep to character id 31
  try
    tell application "Music"
      if theKind is "song" then
        set hits to (search library playlist 1 for theQuery only songs)
      else if theKind is "album" then
        set hits to (search library playlist 1 for theQuery only albums)
      else if theKind is "artist" then
        set hits to (search library playlist 1 for theQuery only artists)
      else if theKind is "playlist" then
        set hits to (user playlists whose name contains theQuery)
      else
        set hits to (search library playlist 1 for theQuery)
      end if
      if hits is missing value then return "none"
      if (count of hits) is 0 then return "none"
      play (item 1 of hits)
      return "ok"
    end tell
  on error errMsg number errNum
    return "error" & sep & errNum
  end try
end run"#;

/// What is playing now: `state`, name, artist. A streaming catalog track has
/// no current track to read, which is an empty answer, not a failure.
const NOW_PLAYING: &str = r#"set sep to character id 31
tell application "Music"
  set s to (player state as string)
  try
    set t to current track
    return s & sep & (name of t) & sep & (artist of t)
  on error
    return s & sep & "" & sep & ""
  end try
end tell"#;

/// AppleScript's "not authorized to send Apple events" error number.
const NOT_AUTHORIZED: i64 = -1743;

/// What the search script said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Search {
    Played,
    NoMatch,
    /// The person has not allowed Juno to control Music.
    NotAllowed,
    /// Something else went wrong; the number is for the log.
    Failed(i64),
}

pub fn parse_search(out: &str) -> Search {
    let mut parts = out.trim().split(SEP);
    match parts.next() {
        Some("ok") => Search::Played,
        Some("none") => Search::NoMatch,
        Some("error") => match parts.next().and_then(|n| n.trim().parse::<i64>().ok()) {
            Some(NOT_AUTHORIZED) => Search::NotAllowed,
            Some(n) => Search::Failed(n),
            None => Search::Failed(0),
        },
        _ => Search::Failed(0),
    }
}

/// A player's state and track, as the read-back script reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playing {
    pub state: String,
    pub track: Option<String>,
    pub artist: Option<String>,
}

pub fn parse_playing(out: &str) -> Playing {
    let mut parts = out.trim().split(SEP);
    let state = parts.next().unwrap_or_default().trim().to_string();
    let field = |p: Option<&str>| {
        p.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Playing {
        state,
        track: field(parts.next()),
        artist: field(parts.next()),
    }
}

/// The sentence for what is now playing.
pub fn now_playing_line(playing: &Playing) -> String {
    match (&playing.track, &playing.artist) {
        (Some(t), Some(a)) => format!("Playing {t} by {a}."),
        (Some(t), None) => format!("Playing {t}."),
        _ => "Playing in Music.".to_string(),
    }
}

pub fn declined() -> Value {
    json!({
        "ok": false,
        "access": "denied",
        "summary": "Juno is not allowed to control Music. You can turn it on in System Settings.",
        "card": format!(
            "<AgendaCard needsAccess=\"Music\" settingsUrl=\"{}\" />",
            urls::AUTOMATION_PANEL
        ),
    })
}

pub fn spotify_in_front() -> Value {
    json!({
        "ok": false,
        "summary": "Spotify is in front and can't be searched, so open Music or switch apps and ask again.",
    })
}

/// Is the app in front Spotify? `lsappinfo` needs no permission.
fn spotify_is_frontmost() -> bool {
    let front = match proc::run("lsappinfo", &["front".to_string()], None, READ_WAIT) {
        Ok(r) if r.ok => r.stdout,
        _ => return false,
    };
    let asked = vec![
        "info".to_string(),
        "-only".to_string(),
        "bundleid".to_string(),
        front,
    ];
    match proc::run("lsappinfo", &asked, None, READ_WAIT) {
        Ok(r) => r.stdout.contains("com.spotify.client"),
        Err(_) => false,
    }
}

/// `music_play`
pub fn play(input: &Value) -> Result<Value, String> {
    let query = required_text(input, "query")?;
    let kind = Kind::parse(text_arg(input, "kind"))?;
    if spotify_is_frontmost() {
        return Ok(spotify_in_front());
    }

    let argv = vec![query.to_string(), kind.word().to_string()];
    let ran = proc::osascript(SEARCH_AND_PLAY, &argv, SEARCH_WAIT)?;
    if ran.timed_out {
        return Ok(json!({
            "ok": false,
            "summary": "Music did not answer. Answer the box on screen if there is one, then ask again.",
        }));
    }
    match parse_search(&ran.stdout) {
        Search::Played => {}
        Search::NoMatch => {
            return Ok(json!({
                "ok": false,
                "summary": format!("I couldn't find {query} in your Apple Music library."),
            }))
        }
        Search::NotAllowed => return Ok(declined()),
        Search::Failed(code) => {
            tracing::warn!("music_play: Music answered with error {code}");
            return Err("Music would not play that.".to_string());
        }
    }

    // Read back what is playing, giving Music a moment to start.
    let mut playing = None;
    for _ in 0..6 {
        thread::sleep(Duration::from_millis(400));
        let seen = proc::osascript(NOW_PLAYING, &[], READ_WAIT)?;
        let parsed = parse_playing(&seen.stdout);
        let started = parsed.state == "playing";
        playing = Some(parsed);
        if started {
            break;
        }
    }
    let playing = playing.ok_or_else(|| "Music did not report what it is playing.".to_string())?;
    if playing.state != "playing" {
        return Err(format!("Music found {query} but did not start it."));
    }
    Ok(json!({
        "ok": true,
        "summary": now_playing_line(&playing),
        "track": playing.track,
        "artist": playing.artist,
        "card": "<NowPlayingCard app=\"Music\" />",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_read_loosely_and_unknown_ones_say_what_to_use() {
        assert_eq!(Kind::parse(None), Ok(Kind::Any));
        assert_eq!(Kind::parse(Some("Album")), Ok(Kind::Album));
        assert_eq!(Kind::parse(Some("band")), Ok(Kind::Artist));
        assert!(Kind::parse(Some("podcast")).is_err());
    }

    #[test]
    fn the_script_is_constant_and_takes_the_query_as_argv() {
        assert!(SEARCH_AND_PLAY.contains("item 1 of argv"));
        assert!(SEARCH_AND_PLAY.contains("only artists"));
        // Nothing from the person is formatted into script text.
        assert!(!SEARCH_AND_PLAY.contains("{}"));
    }

    #[test]
    fn the_search_answer_is_read_by_number_not_by_wording() {
        let denied = format!("error{SEP}-1743");
        assert_eq!(parse_search(&denied), Search::NotAllowed);
        assert_eq!(
            parse_search(&format!("error{SEP}-600")),
            Search::Failed(-600)
        );
        assert_eq!(parse_search("ok\n"), Search::Played);
        assert_eq!(parse_search("none"), Search::NoMatch);
        assert_eq!(parse_search(""), Search::Failed(0));
    }

    #[test]
    fn declined_is_one_sentence_one_action_and_names_no_framework() {
        let d = declined();
        let summary = d["summary"].as_str().unwrap_or_default();
        assert!(!summary.contains("TCC") && !summary.contains("Apple Events"));
        assert_eq!(
            summary.matches('.').count(),
            2,
            "two short sentences at most: {summary}"
        );
        assert!(d["card"]
            .as_str()
            .is_some_and(|c| c.contains("x-apple.systempreferences:")));
    }

    #[test]
    fn spotify_in_front_is_said_in_one_sentence_and_nothing_is_guessed() {
        let s = spotify_in_front();
        assert_eq!(s["ok"], false);
        assert!(s["summary"].as_str().is_some_and(|t| t.contains("Spotify")));
        assert!(s.get("card").is_none());
    }

    #[test]
    fn what_is_playing_reads_back_as_a_sentence() {
        let p = parse_playing(&format!("playing{SEP}Roygbiv{SEP}Boards of Canada\n"));
        assert_eq!(now_playing_line(&p), "Playing Roygbiv by Boards of Canada.");
        let streaming = parse_playing(&format!("playing{SEP}{SEP}"));
        assert_eq!(now_playing_line(&streaming), "Playing in Music.");
        assert_eq!(streaming.state, "playing");
    }

    #[test]
    fn a_missing_query_is_a_plain_error() {
        assert_eq!(play(&json!({})), Err("Missing query.".to_string()));
        assert!(play(&json!({"query": "x", "kind": "podcast"})).is_err());
    }
}
