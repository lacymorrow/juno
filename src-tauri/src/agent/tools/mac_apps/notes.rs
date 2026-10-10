//! Notes: make one, add to one, find one.
//!
//! Constant AppleScript, every value as `argv`. Notes stores a note's body as
//! HTML, so text is escaped here before it goes in, and every write is read
//! back from Notes (its name, its folder, whether the text is in it).

use serde_json::{json, Value};

use super::script::{self, App, Outcome, ERR_NO_SUCH_OBJECT};
use super::*;

/// Most notes one search hands back.
const MAX_NOTES: usize = 10;

/// argv: html body, folder ("" for the default). Answers id, name, folder.
const CREATE_BODY: &str = r#"set h to item 1 of argv
set folderName to item 2 of argv
tell application "Notes"
if folderName is "" then
set n to make new note with properties {body:h}
else
set n to make new note at folder folderName with properties {body:h}
end if
set theId to id of n
set fresh to note id theId
return "OK" & US & (id of fresh) & US & (name of fresh) & US & (name of container of fresh)
end tell"#;

/// argv: note name, html to add, plain text added. Exact name first, then a
/// name that contains it. More than one partial match answers "AMBIGUOUS"
/// and their names; none answers "MISSING". Otherwise id, name, and whether
/// the text now reads back from the note.
const APPEND_BODY: &str = r#"set q to item 1 of argv
set h to item 2 of argv
set t to item 3 of argv
tell application "Notes"
set found to (notes whose name is q)
if (count of found) is 0 then set found to (notes whose name contains q)
if (count of found) is 0 then return "OK" & US & "MISSING"
if (count of found) > 1 then
set out to "OK" & US & "AMBIGUOUS"
repeat with i from 1 to (count of found)
if i > 5 then exit repeat
set out to out & US & (name of item i of found)
end repeat
return out
end if
set n to item 1 of found
set body of n to (body of n) & h
set fresh to note id (id of n)
if (plaintext of fresh) contains t then
set landed to "yes"
else
set landed to "no"
end if
return "OK" & US & "DONE" & US & (id of fresh) & US & (name of fresh) & US & landed
end tell"#;

/// argv: query, limit. Records: id, name, folder, the start of the text.
const SEARCH_BODY: &str = r#"set q to item 1 of argv
set lim to (item 2 of argv) as integer
set out to "OK" & US
tell application "Notes"
set found to (notes whose name contains q or plaintext contains q)
set n to count of found
if n > lim then set n to lim
repeat with i from 1 to n
set m to item i of found
set p to plaintext of m
if (length of p) > 160 then set p to text 1 thru 160 of p
set out to out & (id of m) & US & (name of m) & US & (name of container of m) & US & p & RS
end repeat
end tell
return out"#;

/// Text as Notes HTML: escaped, one `<div>` per line, blank lines kept.
pub fn to_html(text: &str) -> String {
    text.lines()
        .map(|line| {
            let escaped = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");
            if escaped.trim().is_empty() {
                "<div><br></div>".to_string()
            } else {
                format!("<div>{escaped}</div>")
            }
        })
        .collect()
}

/// A new note's body: the title as its heading, then the text.
pub fn note_html(title: &str, body: &str) -> String {
    let heading = to_html(title.trim())
        .replace("<div>", "<div><h1>")
        .replace("</div>", "</h1></div>");
    format!("{heading}{}", to_html(body))
}

/// `notes_create`
pub fn create(input: &Value) -> Result<Value, String> {
    let title = required_text(input, "title")?;
    let body = text_arg(input, "body").unwrap_or_default();
    let folder = text_arg(input, "folder").unwrap_or_default();
    let argv = vec![note_html(title, body), folder.to_string()];
    let fields = match script::run(&script::wrap(CREATE_BODY), argv)? {
        Outcome::Ok(records) => records.into_iter().next().unwrap_or_default(),
        Outcome::Err { number, .. } if number == ERR_NO_SUCH_OBJECT && !folder.is_empty() => {
            return Ok(json!({
                "ok": false,
                "summary": format!("Notes has no folder called {folder}. Nothing was made."),
            }))
        }
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Notes, number, &message))
        }
    };
    let [id, name, folder_name] = match fields.as_slice() {
        [id, name, folder_name, ..] => [id, name, folder_name],
        _ => return Err("Notes made the note but did not say where it is.".to_string()),
    };
    Ok(json!({
        "ok": true,
        "summary": format!("Made a note: {name}, in {folder_name}."),
        "observed": { "id": id, "name": name, "folder": folder_name },
    }))
}

/// `notes_append`
pub fn append(input: &Value) -> Result<Value, String> {
    let note = required_text(input, "note")?;
    let text = required_text(input, "text")?;
    // Notes may fold the first line into its own formatting, so the read back
    // looks for the first non-empty line of what was added.
    let probe = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(text)
        .to_string();
    let argv = vec![note.to_string(), to_html(text), probe];
    let fields = match script::run(&script::wrap(APPEND_BODY), argv)? {
        Outcome::Ok(records) => records.into_iter().next().unwrap_or_default(),
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Notes, number, &message))
        }
    };
    Ok(append_answer(note, &fields))
}

/// What the append script's answer means, worded.
pub fn append_answer(note: &str, fields: &[String]) -> Value {
    match fields {
        [status, ..] if status == "MISSING" => json!({
            "ok": false,
            "summary": format!("There is no note called {note}. Nothing was added."),
        }),
        [status, names @ ..] if status == "AMBIGUOUS" => json!({
            "ok": false,
            "summary": format!("More than one note matches {note}: {}. Which one? Nothing was added.", names.join(", ")),
            "candidates": names,
        }),
        [status, id, name, landed, ..] if status == "DONE" => {
            if landed == "yes" {
                json!({
                    "ok": true,
                    "summary": format!("Added to {name}."),
                    "observed": { "id": id, "name": name },
                })
            } else {
                json!({
                    "ok": false,
                    "summary": format!("Notes took it, but the text does not read back from {name}."),
                    "observed": { "id": id, "name": name },
                })
            }
        }
        _ => json!({ "ok": false, "summary": "Notes did not say whether it was added." }),
    }
}

/// `notes_search`
pub fn search(input: &Value) -> Result<Value, String> {
    let query = required_text(input, "query")?;
    let argv = vec![query.to_string(), MAX_NOTES.to_string()];
    let records = match script::run(&script::wrap(SEARCH_BODY), argv)? {
        Outcome::Ok(records) => records,
        Outcome::Err { number, message } => {
            return Ok(script::error_result(App::Notes, number, &message))
        }
    };
    let notes: Vec<Value> = records
        .iter()
        .filter(|r| r.len() >= 4)
        .map(|r| json!({ "id": r[0], "name": r[1], "folder": r[2], "text": r[3] }))
        .collect();
    let summary = if notes.is_empty() {
        format!("No notes mention {query}.")
    } else {
        notes
            .iter()
            .map(|n| {
                format!(
                    "{} (in {})",
                    n["name"].as_str().unwrap_or_default(),
                    n["folder"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    Ok(json!({ "ok": true, "count": notes.len(), "summary": summary, "notes": notes }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_escaped_before_it_becomes_a_note() {
        assert_eq!(
            to_html("a < b & \"c\""),
            "<div>a &lt; b &amp; &quot;c&quot;</div>"
        );
        assert_eq!(
            to_html("one\n\ntwo"),
            "<div>one</div><div><br></div><div>two</div>"
        );
        assert_eq!(
            note_html("Thursday", "pick up the drawings"),
            "<div><h1>Thursday</h1></div><div>pick up the drawings</div>"
        );
    }

    #[test]
    fn an_append_reports_what_it_found() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(append_answer("Groceries", &s(&["MISSING"]))["ok"], false);
        let many = append_answer("Gro", &s(&["AMBIGUOUS", "Groceries", "Grocery list"]));
        assert_eq!(many["candidates"][1], "Grocery list");
        assert!(many["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("Nothing was added."));
        let done = append_answer(
            "Groceries",
            &s(&["DONE", "x-coredata://1", "Groceries", "yes"]),
        );
        assert_eq!(done["summary"], "Added to Groceries.");
        let unseen = append_answer("Groceries", &s(&["DONE", "id", "Groceries", "no"]));
        assert_eq!(unseen["ok"], false);
    }

    #[test]
    fn every_notes_script_takes_its_values_as_argv() {
        for body in [CREATE_BODY, APPEND_BODY, SEARCH_BODY] {
            let script = script::wrap(body);
            assert!(script.contains("item 1 of argv"), "{body}");
            assert!(!script.contains("{}"), "{body}");
        }
    }
}
