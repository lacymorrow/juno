//! "Report Issue..." — the one box, and everything Juno fills in around it.
//!
//! The person types what happened. Juno supplies the Juno version and the
//! macOS version, because those are the only two things on the bug form that
//! send a reporter away from the form to go and look something up, and they
//! are the two Juno already knows. Nothing else on the form is required
//! (LAC-5069): five required fields is why the repo owner typed `-` into four
//! of them on his own last three issues.
//!
//! The query keys below are the `id:`s in `.github/ISSUE_TEMPLATE/*.yml`.
//! GitHub matches a prefill to a field by that id, so renaming a field there
//! means renaming a key here; `issue_form_url` is unit tested against the ids
//! the templates actually declare.
//!
//! The URL is built here rather than in the page for the usual reason: what a
//! bug report contains is the app's business, the frontend only asks for one
//! to be opened. It also means a headless Juno can file one.

use crate::state::AppState;
use tauri::{AppHandle, State};
use tracing::info;
use url::Url;

/// Where the repo's issue forms live. A bare `/issues/new` with no `template`
/// is a *blank* issue URL: GitHub answers it from `config.yml`, and for most
/// of this repo's life that meant a redirect to `/issues/new/choose` that
/// silently dropped every prefill. Always name the template.
const NEW_ISSUE_URL: &str = "https://github.com/lacymorrow/juno/issues/new";

/// One of the repo's issue forms, and what it can be told in advance.
struct IssueForm {
    /// The template's file name, which is what `?template=` takes.
    template: &'static str,
    /// The id of the form's one required box.
    description_field: &'static str,
    /// Whether the form has `version` and `macos` boxes worth filling in. The
    /// feature form does not: a request is not about one machine.
    takes_environment: bool,
}

const BUG_FORM: IssueForm = IssueForm {
    template: "bug_report.yml",
    description_field: "what-happened",
    takes_environment: true,
};

const FEATURE_FORM: IssueForm = IssueForm {
    template: "feature_request.yml",
    description_field: "problem",
    takes_environment: false,
};

/// Blank is the same as absent. An empty prefill reaches the form as an empty
/// box either way, so leaving the key out loses nothing and keeps the URL
/// short enough to read in a log line.
fn filled(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Build the deep link for one issue form.
fn issue_form_url(
    form: &IssueForm,
    title: Option<&str>,
    description: Option<&str>,
    app_version: Option<&str>,
    macos_version: Option<&str>,
) -> Result<String, String> {
    let mut url = Url::parse(NEW_ISSUE_URL)
        .map_err(|e| format!("Bad issue URL '{}': {}", NEW_ISSUE_URL, e))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("template", form.template);
        if let Some(title) = filled(title) {
            query.append_pair("title", title);
        }
        if let Some(description) = filled(description) {
            query.append_pair(form.description_field, description);
        }
        if form.takes_environment {
            if let Some(app_version) = filled(app_version) {
                query.append_pair("version", app_version);
            }
            if let Some(macos_version) = filled(macos_version) {
                query.append_pair("macos", macos_version);
            }
        }
    }
    Ok(url.into())
}

/// The running macOS version, in the shape the bug form's placeholder shows.
///
/// `NSProcessInfo` is thread safe and answers from the system version already
/// loaded in the process, so this needs neither the main thread nor a
/// `sw_vers` subprocess. A trailing `.0` patch is left off because that is how
/// Apple writes it.
#[cfg(target_os = "macos")]
fn macos_version() -> Option<String> {
    let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
    if version.majorVersion <= 0 {
        return None;
    }
    Some(if version.patchVersion == 0 {
        format!("{}.{}", version.majorVersion, version.minorVersion)
    } else {
        format!(
            "{}.{}.{}",
            version.majorVersion, version.minorVersion, version.patchVersion
        )
    })
}

#[cfg(not(target_os = "macos"))]
fn macos_version() -> Option<String> {
    None
}

/// Open a GitHub issue form with everything Juno already knows filled in.
///
/// `kind` is `"bug"` or `"feature"`. `title` and `description` are whatever the
/// person has already typed, and both may be absent: landing on an empty form
/// is a perfectly good outcome, and better than a dialog asking for them
/// first. Returns the URL it opened, so a caller can log or show it.
#[tauri::command]
pub async fn open_issue_report(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    title: Option<String>,
    description: Option<String>,
) -> Result<String, String> {
    let form = match kind.as_str() {
        "bug" => &BUG_FORM,
        "feature" => &FEATURE_FORM,
        other => {
            return Err(format!(
                "Unknown issue kind '{}'; expected 'bug' or 'feature'",
                other
            ))
        }
    };

    let app_version = format!("v{}", app_handle.package_info().version);
    let url = issue_form_url(
        form,
        title.as_deref(),
        description.as_deref(),
        Some(&app_version),
        macos_version().as_deref(),
    )?;

    info!(
        "[IssueReport] Opening {} ({} chars prefilled)",
        form.template,
        description.as_deref().unwrap_or("").len()
    );

    let desktop = state.get_desktop()?;
    desktop
        .open_url(&url, None)
        .map_err(|e| format!("Failed to open the issue form: {}", e))?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The field ids a prefill keys off are the templates' own. If a template
    /// renames one, this test is the thing that notices.
    #[test]
    fn prefill_keys_are_the_ids_the_templates_declare() {
        let bug = include_str!("../../../.github/ISSUE_TEMPLATE/bug_report.yml");
        for id in ["what-happened", "version", "macos"] {
            assert!(
                bug.contains(&format!("id: {}", id)),
                "bug_report.yml has no field with id '{}'",
                id
            );
        }
        let feature = include_str!("../../../.github/ISSUE_TEMPLATE/feature_request.yml");
        assert!(
            feature.contains("id: problem"),
            "feature_request.yml has no field with id 'problem'"
        );
    }

    /// One required field per form. The whole point of LAC-5069: a reporter in
    /// a hurry fills one box. Adding a second `required: true` undoes it.
    #[test]
    fn each_template_requires_exactly_one_field() {
        for (name, body) in [
            (
                "bug_report.yml",
                include_str!("../../../.github/ISSUE_TEMPLATE/bug_report.yml"),
            ),
            (
                "feature_request.yml",
                include_str!("../../../.github/ISSUE_TEMPLATE/feature_request.yml"),
            ),
        ] {
            let required = body.matches("required: true").count();
            assert_eq!(
                required, 1,
                "{} has {} required fields, want 1",
                name, required
            );
        }
    }

    #[test]
    fn a_bug_report_carries_the_version_and_the_os() {
        let url = issue_form_url(
            &BUG_FORM,
            Some("Dictation drops the first word"),
            Some("I said \"open mail\" and got \"mail\"."),
            Some("v0.8.234"),
            Some("26.0.1"),
        )
        .expect("the bug form URL should build");

        assert!(url.starts_with("https://github.com/lacymorrow/juno/issues/new?"));
        assert!(url.contains("template=bug_report.yml"));
        assert!(url.contains("version=v0.8.234"));
        assert!(url.contains("macos=26.0.1"));
        // Spaces and quotes survive as form encoding, not as raw characters.
        assert!(url.contains("what-happened=I+said"));
        assert!(url.contains("%22open+mail%22"));
    }

    /// A feature request is not about one machine, so it is never asked which
    /// one, and must not carry keys the feature form has no fields for.
    #[test]
    fn a_feature_request_carries_no_machine_details() {
        let url = issue_form_url(
            &FEATURE_FORM,
            Some("Snap the bar to the notch"),
            Some("I want the bar to park in the notch."),
            Some("v0.8.234"),
            Some("26.0.1"),
        )
        .expect("the feature form URL should build");

        assert!(url.contains("template=feature_request.yml"));
        assert!(url.contains("problem=I+want"));
        assert!(!url.contains("version="));
        assert!(!url.contains("macos="));
    }

    /// Nothing typed yet is a normal way to reach the form: the template is
    /// named so GitHub still renders the form rather than redirecting.
    #[test]
    fn an_empty_report_still_names_its_template() {
        let url = issue_form_url(&BUG_FORM, None, Some("   "), None, None)
            .expect("an empty bug form URL should build");
        assert_eq!(
            url,
            "https://github.com/lacymorrow/juno/issues/new?template=bug_report.yml"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_macos_version_reads_as_a_version() {
        let version = macos_version().expect("macOS should report its own version");
        let major = version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok())
            .expect("the first component should be a number");
        assert!(major >= 10, "unexpected macOS major version: {}", version);
    }
}
