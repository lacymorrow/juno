//! Play something in the Music app by name.
//!
//! The library comes first: Music's own scripting search finds the match and
//! plays it; the query travels as `argv`, never as script text. When the
//! library has nothing, Apple's own catalog is searched (the public iTunes
//! Search API, no key) and the match is opened in Music. Opening a catalog
//! item does not always start it, so the answer says what Music shows: playing
//! only when Music reports it, otherwise "it's up in Music, press play". Spotify has no scripting search,
//! so when Spotify is the app in front the answer says so and nothing is
//! guessed (`docs/plans/siri-parity.md`, "Considered and cut").
//!
//! The first call scripts Music, so macOS asks once for permission to control
//! it. Declined is one sentence and the one action that changes it.

use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::maps::link_card;
use super::proc;
use super::{fold, required_text, text_arg};
use crate::constants::permissions::urls;

/// The Automation dialog waits on a person, so the search may take a while.
const SEARCH_WAIT: Duration = Duration::from_secs(120);
const READ_WAIT: Duration = Duration::from_secs(10);

/// How long the catalog lookup may take.
const CATALOG_WAIT: Duration = Duration::from_secs(10);

/// After a play (or an open), how long Music gets to report `playing`. A cloud
/// library track or a catalog item can take several seconds to buffer. This
/// reads back a state the tool just caused, so it is bounded, not open-ended.
const START_TRIES: u32 = 14;
const START_EVERY: Duration = Duration::from_millis(500);

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
///
/// The areas `only` accepts are the whole of Music's search enumeration and
/// nothing else: `albums`, `all`, `artists`, `composers`, `displayed`,
/// `names` (`sdef /System/Applications/Music.app`, macOS 26.6.2). A song
/// title is `names`; iTunes' old `songs` is gone, and asking for it does not
/// fail at run time, it fails to compile, so every song-by-name play errored.
const SEARCH_AND_PLAY: &str = r#"on run argv
  set theQuery to item 1 of argv
  set theKind to item 2 of argv
  set sep to character id 31
  try
    tell application "Music"
      if theKind is "song" then
        set hits to (search library playlist 1 for theQuery only names)
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

// ---------------------------------------------------------------------------
// Apple's catalog
// ---------------------------------------------------------------------------

/// One catalog match: what it is called, whose it is, and its Apple Music page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogHit {
    pub title: String,
    pub artist: String,
    pub url: String,
}

/// The two-letter storefront for a macOS locale id like `en_US` or
/// `en_GB@rg=uszzzz`. Anything unreadable is the US storefront.
pub fn country_from_locale(locale: &str) -> String {
    let base = locale.split('@').next().unwrap_or_default();
    base.rsplit_once('_')
        .map(|(_, region)| region.trim().to_uppercase())
        .filter(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()))
        .unwrap_or_else(|| "US".to_string())
}

fn system_country() -> String {
    let args = vec![
        "read".to_string(),
        "-g".to_string(),
        "AppleLocale".to_string(),
    ];
    match proc::run("defaults", &args, None, READ_WAIT) {
        Ok(r) if r.ok => country_from_locale(r.stdout.trim()),
        _ => "US".to_string(),
    }
}

/// The iTunes Search API URL for a query. An artist is asked for through their
/// songs (`attribute=artistTerm`), because a song is something Music can open
/// and play and an artist page is not.
pub fn catalog_url(query: &str, kind: Kind, by_artist: bool, country: &str) -> String {
    let entity = if kind == Kind::Album { "album" } else { "song" };
    let attribute = if by_artist || kind == Kind::Artist {
        "&attribute=artistTerm"
    } else {
        ""
    };
    format!(
        "https://itunes.apple.com/search?term={}&entity={entity}{attribute}&limit=10&country={}",
        super::maps::encode(query),
        super::maps::encode(country),
    )
}

/// Pick the match from an iTunes Search answer. For an artist, the first song
/// whose artist is the one asked for wins, else the first result.
pub fn parse_catalog(body: &str, query: &str, artist_asked: bool) -> Option<CatalogHit> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    let hits: Vec<CatalogHit> = parsed
        .get("results")?
        .as_array()?
        .iter()
        .filter_map(|r| {
            let text = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
            let url = text("trackViewUrl").or_else(|| text("collectionViewUrl"))?;
            Some(CatalogHit {
                title: text("trackName").or_else(|| text("collectionName"))?,
                artist: text("artistName").unwrap_or_default(),
                url,
            })
        })
        .collect();
    if artist_asked {
        let wanted = fold(query);
        if let Some(hit) = hits.iter().find(|h| fold(&h.artist) == wanted) {
            return Some(hit.clone());
        }
    }
    hits.into_iter().next()
}

/// The `music://` address that opens an Apple Music page in the Music app.
/// Only Apple's own hosts are accepted; the tracking parameter is dropped.
pub fn music_app_url(page: &str) -> Option<String> {
    let rest = page.strip_prefix("https://")?;
    let (host_and_path, query) = match rest.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let host = host_and_path.split('/').next().unwrap_or_default();
    if host != "music.apple.com" && host != "itunes.apple.com" {
        return None;
    }
    let kept: Vec<&str> = query
        .map(|q| q.split('&').filter(|p| !p.starts_with("uo=")).collect())
        .unwrap_or_default();
    Some(if kept.is_empty() {
        format!("music://{host_and_path}")
    } else {
        format!("music://{host_and_path}?{}", kept.join("&"))
    })
}

/// Look the query up in Apple's catalog. Songs by name first; when that finds
/// nothing and the kind was open, songs by an artist of that name.
fn catalog_lookup(query: &str, kind: Kind) -> Option<CatalogHit> {
    // A catalog has no playlists a name search can reach.
    if kind == Kind::Playlist {
        return None;
    }
    let country = system_country();
    let attempts = if kind == Kind::Any {
        vec![(false, false), (true, true)]
    } else {
        vec![(kind == Kind::Artist, kind == Kind::Artist)]
    };
    for (by_artist, artist_asked) in attempts {
        let url = catalog_url(query, kind, by_artist, &country);
        let args = vec![
            "-s".to_string(),
            "--max-time".to_string(),
            "8".to_string(),
            url,
        ];
        let Ok(ran) = proc::run("curl", &args, None, CATALOG_WAIT) else {
            continue;
        };
        if !ran.ok {
            continue;
        }
        if let Some(hit) = parse_catalog(&ran.stdout, query, artist_asked) {
            return Some(hit);
        }
    }
    None
}

/// Let Music report `playing`, up to a bound. `expected` is the catalog title
/// when one was opened; the answer then only counts if Music shows that track.
fn read_back(expected: Option<&str>) -> Result<Playing, String> {
    let mut last = None;
    for attempt in 0..START_TRIES {
        if attempt > 0 {
            thread::sleep(START_EVERY);
        }
        let seen = proc::osascript(NOW_PLAYING, &[], READ_WAIT)?;
        let parsed = parse_playing(&seen.stdout);
        let started = parsed.state == "playing" && is_expected(&parsed, expected);
        last = Some(parsed);
        if started {
            break;
        }
    }
    last.ok_or_else(|| "Music did not report what it is playing.".to_string())
}

/// Is what Music shows the thing that was opened? With nothing expected, any
/// track is. A catalog stream may name no track at all, which cannot be checked.
fn is_expected(playing: &Playing, expected: Option<&str>) -> bool {
    match (expected, playing.track.as_deref()) {
        (Some(want), Some(have)) => {
            let (want, have) = (fold(want), fold(have));
            have.contains(&want) || want.contains(&have)
        }
        _ => true,
    }
}

/// The sentence when Music was asked and is not playing. Says what Music shows.
pub fn not_playing_line(playing: &Playing, query: &str) -> String {
    match &playing.track {
        Some(t) => format!("Music has {t} up but it is {}. Press play.", playing.state),
        None => format!(
            "Music has {query} up but it is {}. Press play.",
            playing.state
        ),
    }
}

/// Open a catalog hit in Music and say what Music then shows.
fn open_catalog_hit(hit: &CatalogHit) -> Result<Value, String> {
    let Some(address) = music_app_url(&hit.url) else {
        return Ok(json!({
            "ok": false,
            "summary": format!("I found {} in Apple Music but could not open it.", hit.title),
        }));
    };
    let ran = proc::run(
        "open",
        &["-a".to_string(), "Music".to_string(), address.clone()],
        None,
        READ_WAIT,
    )?;
    if !ran.ok {
        return Err("Music would not open that.".to_string());
    }
    let playing = read_back(Some(&hit.title))?;
    let title = format!("{} by {}", hit.title, hit.artist);
    if playing.state == "playing" && is_expected(&playing, Some(&hit.title)) {
        return Ok(json!({
            "ok": true,
            "summary": format!("Playing {title} from Apple Music."),
            "observed": { "app": "Music", "state": playing.state, "track": playing.track, "artist": playing.artist },
            "card": "<NowPlayingCard app=\"Music\" />",
        }));
    }
    Ok(json!({
        "ok": true,
        "summary": format!("{title} is up in Music. Press play."),
        "observed": { "app": "Music", "state": playing.state, "track": playing.track, "artist": playing.artist, "opened": hit.title },
        "card": link_card(&address, &title),
    }))
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
            return match catalog_lookup(query, kind) {
                Some(hit) => open_catalog_hit(&hit),
                None => Ok(json!({
                    "ok": false,
                    "summary": format!("I couldn't find {query} in your library or in Apple Music."),
                })),
            }
        }
        Search::NotAllowed => return Ok(declined()),
        Search::Failed(code) => {
            tracing::warn!("music_play: Music answered with error {code}");
            return Err("Music would not play that.".to_string());
        }
    }

    let playing = read_back(None)?;
    if playing.state != "playing" {
        // Music was told to play and shows something else: say what it shows.
        return Ok(json!({
            "ok": false,
            "summary": not_playing_line(&playing, query),
            "observed": { "app": "Music", "state": playing.state, "track": playing.track, "artist": playing.artist },
        }));
    }
    Ok(json!({
        "ok": true,
        "summary": now_playing_line(&playing),
        "track": playing.track,
        "artist": playing.artist,
        "observed": { "app": "Music", "state": playing.state, "track": playing.track, "artist": playing.artist },
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

    /// An area Music does not have is a compile error, which no run-time
    /// handling here can soften: the whole script dies before it searches.
    #[test]
    fn every_search_area_is_one_music_knows() {
        // The whole enumeration, from `sdef /System/Applications/Music.app`.
        const AREAS: [&str; 6] = [
            "albums",
            "all",
            "artists",
            "composers",
            "displayed",
            "names",
        ];
        let asked: Vec<&str> = SEARCH_AND_PLAY
            .match_indices("only ")
            .map(|(at, marker)| {
                SEARCH_AND_PLAY[at + marker.len()..]
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .next()
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(asked, ["names", "albums", "artists"]);
        for area in asked {
            assert!(AREAS.contains(&area), "Music has no search area {area:?}");
        }
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

    const ITUNES: &str = r#"{"resultCount":2,"results":[
{"wrapperType":"track","kind":"song","artistName":"Someone Else","trackName":"Other","trackViewUrl":"https://music.apple.com/us/album/other/1?i=2&uo=4"},
{"wrapperType":"track","kind":"song","artistName":"Flume","trackName":"The Difference","trackViewUrl":"https://music.apple.com/us/album/the-difference/1694793030?i=1694793033&uo=4"}]}"#;

    #[test]
    fn the_catalog_url_is_encoded_and_asks_for_songs() {
        assert_eq!(
            catalog_url("Jurassic 5", Kind::Artist, false, "US"),
            "https://itunes.apple.com/search?term=Jurassic%205&entity=song&attribute=artistTerm&limit=10&country=US"
        );
        assert_eq!(
            catalog_url("Roygbiv & co", Kind::Song, false, "GB"),
            "https://itunes.apple.com/search?term=Roygbiv%20%26%20co&entity=song&limit=10&country=GB"
        );
        assert!(catalog_url("Geogaddi", Kind::Album, false, "US").contains("entity=album"));
    }

    #[test]
    fn the_storefront_comes_from_the_system_locale() {
        assert_eq!(country_from_locale("en_US"), "US");
        assert_eq!(country_from_locale("en_GB@rg=uszzzz"), "GB");
        assert_eq!(country_from_locale("fr"), "US");
        assert_eq!(country_from_locale(""), "US");
    }

    #[test]
    fn an_itunes_answer_reads_into_one_hit() {
        let first = parse_catalog(ITUNES, "something", false);
        assert_eq!(first.map(|h| h.title), Some("Other".to_string()));
        let flume = parse_catalog(ITUNES, "Flume", true);
        assert_eq!(
            flume,
            Some(CatalogHit {
                title: "The Difference".to_string(),
                artist: "Flume".to_string(),
                url: "https://music.apple.com/us/album/the-difference/1694793030?i=1694793033&uo=4"
                    .to_string(),
            })
        );
        assert_eq!(
            parse_catalog("{\"resultCount\":0,\"results\":[]}", "x", false),
            None
        );
        assert_eq!(parse_catalog("not json", "x", false), None);
    }

    #[test]
    fn only_apples_own_pages_open_in_music() {
        assert_eq!(
            music_app_url("https://music.apple.com/us/album/a/1?i=2&uo=4"),
            Some("music://music.apple.com/us/album/a/1?i=2".to_string())
        );
        assert_eq!(
            music_app_url("https://music.apple.com/us/artist/a/1?uo=4"),
            Some("music://music.apple.com/us/artist/a/1".to_string())
        );
        assert_eq!(music_app_url("https://evil.example/a?i=2"), None);
        assert_eq!(music_app_url("http://music.apple.com/x"), None);
    }

    #[test]
    fn a_track_that_is_not_the_one_opened_does_not_count() {
        let other = Playing {
            state: "playing".to_string(),
            track: Some("Some Old Song".to_string()),
            artist: None,
        };
        assert!(!is_expected(&other, Some("The Difference")));
        assert!(is_expected(&other, None));
        let same = Playing {
            track: Some("The Difference (feat. Toro y Moi)".to_string()),
            ..other
        };
        assert!(is_expected(&same, Some("The Difference")));
    }

    #[test]
    fn a_stopped_player_is_reported_as_what_music_shows() {
        let p = Playing {
            state: "paused".to_string(),
            track: Some("Roygbiv".to_string()),
            artist: None,
        };
        assert_eq!(
            not_playing_line(&p, "x"),
            "Music has Roygbiv up but it is paused. Press play."
        );
    }
}
