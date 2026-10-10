//! Reading the Messages history, read only.
//!
//! Messages keeps every conversation in `~/Library/Messages/chat.db`, a SQLite
//! file. Juno opens it read only through the system `sqlite3` (no copy is made,
//! nothing is written), and only after the file itself opened, which is the
//! one honest test for Full Disk Access: macOS refuses the open when Juno does
//! not have it.
//!
//! Newer rows keep the text inside `attributedBody`, an archived
//! `NSAttributedString` in Apple's old typedstream format, with `text` null.
//! [`decode_attributed_body`] pulls the string out; it is pure and tested on a
//! blob built by hand.
//!
//! SQL here is constant. The only values ever written into a query are
//! integers this file produced (row ids, a timestamp), never anything a person
//! or a model said.

use std::path::PathBuf;

use serde_json::Value;

use super::send::Evidence;

/// Seconds from 1970-01-01 to 2001-01-01, Apple's epoch.
const APPLE_EPOCH_OFFSET: i64 = 978_307_200;

// ---------------------------------------------------------------------------
// Pure: the body, the clock
// ---------------------------------------------------------------------------

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The string inside an `attributedBody` blob.
///
/// The archive names its classes, then the string: `NSString`, a few type
/// bytes ending in `+` (0x2B, "a C string follows"), a length, the UTF-8 bytes.
/// The length is one byte below 0x80, or 0x81 then two bytes little endian, or
/// 0x82 then four. Anything else is not a shape this knows, and comes back
/// `None` rather than a guess.
pub fn decode_attributed_body(blob: &[u8]) -> Option<String> {
    let after = [b"NSString".as_slice(), b"NSMutableString".as_slice()]
        .iter()
        .find_map(|marker| find(blob, marker).map(|at| at + marker.len()))?;
    let plus = blob
        .get(after..)?
        .iter()
        .take(16)
        .position(|b| *b == 0x2B)?;
    let mut at = after + plus + 1;
    let first = *blob.get(at)?;
    at += 1;
    let length = match first {
        0x81 => {
            let bytes = blob.get(at..at + 2)?;
            at += 2;
            usize::from(u16::from_le_bytes([bytes[0], bytes[1]]))
        }
        0x82 => {
            let bytes = blob.get(at..at + 4)?;
            at += 4;
            usize::try_from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).ok()?
        }
        n if n < 0x80 => usize::from(n),
        _ => return None,
    };
    let text = blob.get(at..at.checked_add(length)?)?;
    String::from_utf8(text.to_vec()).ok()
}

/// SQLite's `hex()` back to bytes.
pub fn from_hex(hex: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = hex.trim().bytes().collect();
    if digits.len() % 2 != 0 {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16)?;
            let lo = (pair[1] as char).to_digit(16)?;
            u8::try_from(hi * 16 + lo).ok()
        })
        .collect()
}

/// `message.date` to Unix seconds. Older databases store seconds since 2001,
/// newer ones nanoseconds.
pub fn apple_to_unix(raw: i64) -> i64 {
    let secs = if raw.abs() > 100_000_000_000 {
        raw / 1_000_000_000
    } else {
        raw
    };
    secs + APPLE_EPOCH_OFFSET
}

/// Unix seconds to the nanosecond `message.date` newer databases use.
pub fn unix_to_apple_ns(unix: i64) -> i64 {
    (unix - APPLE_EPOCH_OFFSET).saturating_mul(1_000_000_000)
}

/// One message row, decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub rowid: i64,
    pub from_me: bool,
    /// Unix seconds.
    pub at: i64,
    pub text: String,
    pub is_sent: bool,
    pub is_delivered: bool,
    pub error: i64,
    pub handle_id: i64,
}

fn int(row: &Value, key: &str) -> i64 {
    row.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// A row from `sqlite3 -json`, with the body taken from `text` or, when that
/// is empty, from `attributedBody`.
pub fn row_from_json(row: &Value) -> Row {
    let plain = row
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|t| !t.trim().is_empty());
    let text = plain
        .or_else(|| {
            row.get("body_hex")
                .and_then(Value::as_str)
                .and_then(from_hex)
                .and_then(|blob| decode_attributed_body(&blob))
        })
        .unwrap_or_default();
    Row {
        rowid: int(row, "rowid"),
        from_me: int(row, "from_me") != 0,
        at: apple_to_unix(int(row, "date")),
        text,
        is_sent: int(row, "is_sent") != 0,
        is_delivered: int(row, "is_delivered") != 0,
        error: int(row, "error"),
        handle_id: int(row, "handle_id"),
    }
}

/// Fold a body for comparing what was sent with what was read back: Messages
/// can swap straight quotes for curly ones and trims the ends.
fn same_body(a: &str, b: &str) -> bool {
    let fold = |s: &str| {
        s.trim()
            .chars()
            .map(|c| match c {
                '\u{2018}' | '\u{2019}' => '\'',
                '\u{201c}' | '\u{201d}' => '"',
                other => other,
            })
            .collect::<String>()
    };
    fold(a) == fold(b)
}

/// What the read-back rows say about the message just sent. `None` when it is
/// not there (yet).
pub fn evidence_for(rows: &[Row], body: &str) -> Option<Evidence> {
    let row = rows
        .iter()
        .filter(|r| r.from_me)
        .find(|r| same_body(&r.text, body))?;
    Some(if row.error != 0 {
        Evidence::Failed
    } else if row.is_sent || row.is_delivered {
        Evidence::Confirmed
    } else {
        Evidence::Waiting
    })
}

/// A list of integers for an `IN (...)`. Only integers ever reach SQL.
pub fn id_list(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

// ---------------------------------------------------------------------------
// The file
// ---------------------------------------------------------------------------

/// Whether the history can be read.
#[derive(Debug)]
pub enum Access {
    Readable(PathBuf),
    /// macOS refused the open: Full Disk Access is off.
    NoAccess,
    /// There is no history on this Mac.
    Missing,
}

pub fn path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Library").join("Messages").join("chat.db"))
}

/// Try the open. The kind of error is the answer, not its text.
pub fn access() -> Access {
    let Some(path) = path() else {
        return Access::Missing;
    };
    match std::fs::File::open(&path) {
        Ok(_) => Access::Readable(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Access::Missing,
        Err(_) => Access::NoAccess,
    }
}

/// Run one constant query, read only, and hand back the rows.
pub fn query(path: &std::path::Path, sql: &str) -> Result<Vec<Value>, String> {
    let output = std::process::Command::new("/usr/bin/sqlite3")
        .arg("-readonly")
        .arg("-json")
        .arg(path)
        .arg(sql)
        .output()
        .map_err(|e| format!("Could not read the Messages history: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not read the Messages history: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<Value>>(&text)
        .map_err(|e| format!("Could not read the Messages history: {e}"))
}

/// Every handle Messages knows: (row id, address, service).
pub fn handles(path: &std::path::Path) -> Result<Vec<(i64, String, String)>, String> {
    let rows = query(
        path,
        "SELECT ROWID AS rowid, id AS handle, service FROM handle;",
    )?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                int(r, "rowid"),
                r.get("handle")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                r.get("service")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect())
}

const MESSAGE_COLUMNS: &str = "m.ROWID AS rowid, m.is_from_me AS from_me, m.date AS date, \
     m.text AS text, hex(m.attributedBody) AS body_hex, m.is_sent AS is_sent, \
     m.is_delivered AS is_delivered, m.error AS error, m.handle_id AS handle_id";

/// Ordinary messages only: no tapbacks, no "named the conversation" events.
const ORDINARY: &str =
    "COALESCE(m.associated_message_type, 0) = 0 AND COALESCE(m.item_type, 0) = 0";

/// Messages received, newest first, from these handles or (empty) from anyone.
pub fn received(
    path: &std::path::Path,
    handle_ids: &[i64],
    limit: usize,
) -> Result<Vec<Row>, String> {
    let from = if handle_ids.is_empty() {
        String::new()
    } else {
        format!(" AND m.handle_id IN ({})", id_list(handle_ids))
    };
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM message m WHERE m.is_from_me = 0 AND {ORDINARY}{from} \
         ORDER BY m.date DESC LIMIT {};",
        limit.clamp(1, 50)
    );
    Ok(query(path, &sql)?.iter().map(row_from_json).collect())
}

/// What Juno sent to these handles since a moment (Unix seconds), newest first.
pub fn sent_since(
    path: &std::path::Path,
    handle_ids: &[i64],
    since_unix: i64,
) -> Result<Vec<Row>, String> {
    if handle_ids.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM message m WHERE m.is_from_me = 1 AND {ORDINARY} \
         AND m.handle_id IN ({}) AND (m.date >= {} OR (m.date < 100000000000 AND m.date >= {})) \
         ORDER BY m.date DESC LIMIT 10;",
        id_list(handle_ids),
        unix_to_apple_ns(since_unix),
        since_unix - APPLE_EPOCH_OFFSET,
    );
    Ok(query(path, &sql)?.iter().map(row_from_json).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `attributedBody` the way Messages writes one, built by hand: the
    /// typedstream header, the class chain, then the string.
    fn blob(text: &str) -> Vec<u8> {
        let mut out: Vec<u8> = vec![0x04, 0x0B];
        out.extend_from_slice(b"streamtyped");
        out.extend_from_slice(&[0x81, 0xE8, 0x03, 0x84, 0x01, 0x40, 0x84, 0x84, 0x84, 0x12]);
        out.extend_from_slice(b"NSAttributedString");
        out.extend_from_slice(&[0x00, 0x84, 0x84, 0x08]);
        out.extend_from_slice(b"NSObject");
        out.extend_from_slice(&[0x00, 0x85, 0x92, 0x84, 0x84, 0x84, 0x08]);
        out.extend_from_slice(b"NSString");
        out.extend_from_slice(&[0x01, 0x94, 0x84, 0x01, 0x2B]);
        let bytes = text.as_bytes();
        if bytes.len() < 0x80 {
            out.push(u8::try_from(bytes.len()).unwrap_or(0));
        } else {
            out.push(0x81);
            out.extend_from_slice(&u16::try_from(bytes.len()).unwrap_or(0).to_le_bytes());
        }
        out.extend_from_slice(bytes);
        // What follows the string in a real archive: its attributes.
        out.extend_from_slice(&[
            0x86, 0x84, 0x02, 0x69, 0x49, 0x01, 0x0A, 0x92, 0x84, 0x84, 0x84,
        ]);
        out.extend_from_slice(b"NSDictionary");
        out
    }

    #[test]
    fn a_short_body_comes_out_of_the_archive() {
        assert_eq!(
            decode_attributed_body(&blob("I'm running late")).as_deref(),
            Some("I'm running late")
        );
    }

    #[test]
    fn a_long_body_with_emoji_uses_the_two_byte_length() {
        let long = format!("{} \u{1F44D} done", "word ".repeat(40));
        assert!(long.len() > 0x80);
        assert_eq!(decode_attributed_body(&blob(&long)), Some(long));
    }

    #[test]
    fn garbage_is_none_not_a_guess() {
        assert_eq!(decode_attributed_body(b"no string here"), None);
        let mut truncated = blob("hello there");
        truncated.truncate(truncated.len() - 30);
        assert_eq!(decode_attributed_body(&truncated), None);
        assert_eq!(decode_attributed_body(&[]), None);
    }

    #[test]
    fn hex_from_sqlite_round_trips() {
        let original = blob("Pick up the drawings");
        let hex: String = original.iter().map(|b| format!("{b:02X}")).collect();
        assert_eq!(from_hex(&hex), Some(original));
        assert_eq!(from_hex("ABC"), None);
        assert_eq!(from_hex("ZZ"), None);
    }

    #[test]
    fn a_row_with_null_text_reads_the_archive() {
        let hex: String = blob("On my way")
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        let row = row_from_json(&serde_json::json!({
            "rowid": 7, "from_me": 1, "date": 781_000_000_000_000_000i64,
            "text": null, "body_hex": hex, "is_sent": 1, "is_delivered": 0,
            "error": 0, "handle_id": 3
        }));
        assert_eq!(row.text, "On my way");
        assert!(row.from_me && row.is_sent);
        assert_eq!(row.at, 781_000_000 + 978_307_200);
        let plain = row_from_json(&serde_json::json!({"text": "hi", "body_hex": ""}));
        assert_eq!(plain.text, "hi");
    }

    #[test]
    fn both_clock_formats_read_the_same() {
        assert_eq!(apple_to_unix(781_000_000), 781_000_000 + 978_307_200);
        assert_eq!(
            apple_to_unix(781_000_000_000_000_000),
            781_000_000 + 978_307_200
        );
        assert_eq!(
            apple_to_unix(unix_to_apple_ns(1_791_550_800)),
            1_791_550_800
        );
    }

    fn sent(text: &str, is_sent: bool, error: i64) -> Row {
        Row {
            rowid: 1,
            from_me: true,
            at: 0,
            text: text.to_string(),
            is_sent,
            is_delivered: false,
            error,
            handle_id: 1,
        }
    }

    #[test]
    fn read_back_decides_what_juno_may_say() {
        let body = "I'm running late";
        assert_eq!(
            evidence_for(&[sent("I\u{2019}m running late", true, 0)], body),
            Some(Evidence::Confirmed)
        );
        assert_eq!(
            evidence_for(&[sent(body, false, 0)], body),
            Some(Evidence::Waiting)
        );
        assert_eq!(
            evidence_for(&[sent(body, false, 22)], body),
            Some(Evidence::Failed)
        );
        assert_eq!(evidence_for(&[sent("something else", true, 0)], body), None);
        let mut theirs = sent(body, true, 0);
        theirs.from_me = false;
        assert_eq!(evidence_for(&[theirs], body), None);
    }

    #[test]
    fn only_integers_reach_sql() {
        assert_eq!(id_list(&[3, 14, 15]), "3,14,15");
        assert_eq!(id_list(&[]), "");
    }
}
