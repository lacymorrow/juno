# Display Window System

Juno supports lightweight visual windows that can be spawned, updated or closed by the AI agent.  These are useful for showing pictures, weather widgets, calendars, dashboards, or any HTML content.

## Markup (`<DISPLAY>`)

Inside any assistant response you can embed one or more XML-like blocks:

```xml
<DISPLAY id="cats_pic" kind="image" title="Random Cat" pos="100,100" size="400,300">
file://~/Pictures/cat.jpg
</DISPLAY>
```

Attribute | Description | Required
--------- | ----------- | --------
`id`      | Unique handle for future updates/closure | ✓
`kind`    | `image | widget | html | url`            | ✓
`title`   | Window caption                           | –
`pos`     | `x,y` screen coordinates                 | –
`size`    | `w,h` dimensions in pixels               | –
`autoUpdate` | Interval in ms to auto refresh (future) | –

The block content is the payload:
* **image** – base64, file path, or URL
* **widget** – JSON parameters (see below)
* **html** – raw HTML string
* **url** – URL string

## Widgets

Kind `widget` accepts a JSON object describing the widget type and parameters:

### Weather Widget
```json
{"widget":"weather","location":"San Francisco, CA"}
```
*Uses wttr.in for data (no API key required).*  Displays current temperature and summary.

### Calendar Widget
```json
{"widget":"calendar","url":"https://calendar.google.com/calendar/embed?..."}
```
Embeds the provided calendar URL in an iframe.

More widgets can be added by extending `display.html`.

## Tool Commands

For imperative control the agent can issue these tools (batched calls encouraged):

Command | Purpose | Args
------- | ------- | ----
`display_spawn`  | Create a new window | `{id, kind, payload, title?, position?, size?}`
`display_update` | Update window content | `{id, payload}`
`display_close`  | Close a window        | `{id}`

## Security

* Payload size limited to **5 MB**.
* Only the four allowed kinds are accepted; others raise error.
* HTML payload sanitized via `ammonia` to strip scripts, inline JS, etc.
* External URLs are loaded in sandboxed iframes.

## Development Notes
* State stored in `DisplayWindowManager` (DashMap) managed by Tauri.
* Frontend implementation in `src/display.html` listens for `display://init` and `display://update` events.
* Rust commands in `src-tauri/src/commands/display.rs`.

## Testing

Unit tests located in `src-tauri/tests/display_tests.rs` cover validation and sanitization logic.

```bash
cargo test --package juno --test display_tests
```