# Redesign audit of every agent component

Lacy, 2026-10-15: "a full redesign audit ... for the task card [and] all of the pre-created components."

This is an audit and a spec. No component source changes in this PR. The one code change is the bench that produced the evidence: `/__component-gallery` (`src/bar-harness/ComponentGallery.tsx`), which renders every tag in `availableComponents` through the real `MixedContentRenderer` on each chat surface, with the bar bench's fake Tauri layer so the live cards have something to read.

Media: `docs/changelog/media/822/` (`before-*` from the gallery, `mockup-*` from a static HTML page of the proposed set).

## Verdict

**REDO the set, not the idea.** The agent-rendered component is the right product: one answer, one card, one action. What ships today is three generations of components under one registry (a 2024 "JSX visual capabilities demo", a 2025 animated kit with glows and confetti, and the 2026 flat Mac cards `AgendaCard`, `MessageCard`, `PermissionAskCard`) plus two layout bugs that break the prompt's own examples on every surface. Of 38 registered custom tags, 12 ship as they are or with polish, 10 get redone on a shared primitive set, and 16 are cut or merged. The task card is the first redo: it is the card the prompt reaches for after every long task, it is purple, it strikes through the work it just did, and on the main window it is only as wide as its longest line.

For: the person who just said "organize my downloads", in the three seconds after Juno says "Done".

### The top five changes

1. **Fix the two layout bugs under every card** (S, P0). `MessageContent` is `w-fit`, so every card is as wide as its longest line and a bare progress bar is 0px wide; the segment splitter only sees capitalised tags, so the prompt's `<div className="flex gap-2 mt-2">` button row is dropped and the buttons stack. Both are one-line fixes and the prompt examples start working.
2. **One primitive set, one accent** (M). Card surface, header row, list row, meter, stat, button, skeleton, access row: eight primitives, system blue the only accent, SF, sentence case, hairlines. Every domain card is rebuilt on them. Island gets them edge to edge with no inner card.
3. **Redo the task card** (S, on top of 2). Blue check circles, no strikethrough, "2 of 3" in the header, a 4px meter, actions in a card footer, one primary.
4. **Cut the demo kit and the effects** (S). `GlowBadge`, `ShimmerText`, `PulseRing`, `Confetti`, `AnimatedDivider`, `Circle`, `Rectangle`, `Triangle`, `ColorShowcase`, `VisualDemo`, `StatusCard`, `ProgressBar`, `ActionButton`, the `glow` prop, and the dead passthrough controls (`Input`, `Textarea`, `Switch`, `Select`, `Dialog`, `Tooltip`, `Skeleton`, bare `Button`). Then remove them from `templates.rs`.
5. **Flip the prompt from "compose with Tailwind" to "pick the card"** (S, `templates.rs`). The "COMPOSE RATHER THAN CONFIGURE" section is where the uppercase tracked labels, coloured badges, yellow humidity bars and `text-4xl font-bold` come from. The fix for all-or-nothing streaming is a good skeleton, not hand-rolled layouts.

## Amendment (2026-10-16, Lacy): shadcn first

The set was always meant to be built on shadcn/ui, and the app should stay feature-filled. This overrides the sections below where they disagree.

- **Primitives are restyled shadcn, not new components.** Card is `ui/card.tsx` (`CardHeader`, `CardTitle`, `CardAction`, `CardContent`, `CardFooter`). Meter is `ui/progress.tsx`. Button is `ui/button.tsx` (add a `size="card"` at 24px if needed). Skeleton is `ui/skeleton.tsx`. MiniChart is rebuilt on `ui/chart.tsx`. List rows and Stat are thin compositions inside `CardContent`. The look comes from tokens, not from bespoke markup.
- **Tokens are scoped.** The Mac token set (SF, system blue `--primary`, label and fill scale, hairline) applies under the agent-content scope (main window messages, Pill pane, Island face), so every shadcn part rendered there inherits it. Re-theming the whole app is a separate call for Lacy.
- **shadcn passthroughs stay registered.** Keep `Card*`, `Alert*`, `Badge`, `Separator`, `Tabs*`, `Tooltip*`, `Skeleton`, `Button`, and add `Table*`, `Accordion*`, `Progress`, `Avatar`. They inherit the tokens. Form controls (`Input`, `Textarea`, `Switch`, `Select*`, `Checkbox`, `Slider`) stay registered but are not taught in the prompt until a `<Form>` sends their values as the next turn (follow-up step 10); a control that does nothing is the dead-control defect.
- **Cuts that stand:** the demo kit (`Circle`, `Rectangle`, `Triangle`, `ColorShowcase`, `VisualDemo`, `StatusCard`, `ProgressBar`), the effects (`GlowBadge`, `ShimmerText`, `PulseRing`, `Confetti`, `AnimatedDivider`), and `ActionButton`. None are shadcn. The Animated* merges stand.
- **Step 10 (new):** `<Form>` wrapper: shadcn controls inside it, one primary submit, values sent as the person's next message.

## Method

- Rendered: every entry in `availableComponents` (`src/components/ui/jsx-message-renderer.tsx:360`), in every state the markup can put it in (populated, empty, overflow and long text, open tag still streaming, shell open and children streaming, closed-but-malformed fallback, thrown error, light, dark), on three surfaces: the main window at 720px (light and dark), the Pill's 419px dark pane, and the Island's 360px black component face from `feat/island-one-surface` (#818, header 36px, body `px-2`, `text-[13px]`, `.dark` scope). `NowPlayingCard` was driven through playing, paused, stopped, not running, loading, and error.
- Not rendered: a real player, a real file on disk (the ImageCard shows a generated SVG), `PermissionAskCard` with its button (`onContinue` is a function prop the markup path cannot pass, so the agent-emitted tag never shows the button; only the approval prompt does), `MessageCard` with `onSend` (same), and Reduce Motion.
- Lenses: `insanely-great` (the demo test, applied per verdict), `impeccable` critique (single context; the mechanical detector ran clean over the six source files, which is itself a finding: it cannot see colour-as-meaning, infinite animation or fake state), `apple-hig` widgets and Live Activities (16pt margins, 11pt minimum text, semantic colour, "convey meaning via text and iconography, not colour alone", animations under 2s, "prefer one interactive element").
- Bench: `bunx vite --port 1453`, then `http://localhost:1453/__component-gallery?surface=island&c=TaskSummaryCard`. Query documented at the top of the file.

## System-level findings

These are ordered by how many components they break. Fix them first; several per-component problems disappear with them.

### S1. Cards are only as wide as their longest line (P0)

`src/components/ai-elements/message.tsx:60` wraps every assistant message in `w-fit min-w-0 max-w-full`. A card with short labels is 240px wide; the same card with a long label is 700px; two cards in one answer are two different widths (`before-task-card-main-light.png`, the `populated` vs `overflow` tiles). A bare `<AnimatedProgress>` with no label has no text to size against and renders at 0px (`before-animated-progress-main-light.png`, the `bare` tile). The Pill pane and the Island do not have this wrapper, which is why the same card looks right there and wrong in the main window.

Fix: assistant `MessageContent` gets `w-full` (the user bubble keeps `w-fit`). Cards then fill the column, as every chat product and every HIG widget does.

### S2. The splitter drops lowercase wrappers, so the prompt's examples break (P0)

`JSX_OPEN_TAG_PATTERN` (`src/lib/mixedContent.ts:25`) matches `[A-Z]` tags only. A top-level `<div className="flex gap-2 mt-2">` around two buttons is prose; the buttons become two segments in a `space-y-3` column and stack vertically (`before-task-card-main-light.png` last tile, `before-buttons-main-light.png`). The prompt teaches this exact shape in its Task Completion example (`templates.rs:997-1006`) and teaches `<div className="flex justify-around">` for Stat rows. Inside a capitalised wrapper (`<AnimatedCard>`) the div survives, which is why the composed examples half-work.

Fix: either let the splitter open a block on a lowercase tag that contains a capitalised tag, or (simpler and better) give buttons a home: a card footer slot and a `<Actions>` tag. Then delete the div pattern from the prompt.

### S3. Three design languages, one registry

- 2024 demo kit (`jsx-message-renderer.tsx:122-358`): `Circle`, `Rectangle`, `Triangle`, `ColorShowcase`, `StatusCard`, `ProgressBar`, `VisualDemo`. Hard-coded `text-gray-600`, `bg-blue-600`, `border: 2px solid black`, and a caption under every shape that reads "Circle: 60×60px, lightblue with navy border". Nothing in the prompt's channel table ever asks for them.
- 2025 animated kit (`animated-components.tsx`, `agent-cards/index.tsx`): gradients, `hover:shadow-md`, purple, green and yellow as structure, a 0.5s `juno-animate-in` on every element with staggered delays, infinite glow, shimmer, float, rain and snow pseudo-elements.
- 2026 Mac cards (`agenda-card.tsx`, `message-card.tsx`, `permission-ask-card.tsx`): flat, `#0a84ff` the only accent, hairline dividers, no entry animation, "Flat on purpose" in the header comment.

The third is the bar. The spec below is that language extended to the rest.

### S4. Colour carries meaning it does not have

- `TaskSummaryCard`: purple icon, purple meter, purple arrow; the meter turns green at 100%.
- `AnimatedProgress color="auto"` and `SystemStatusCard`'s `MetricBar`: green under 70, yellow to 90, red above. Humidity 82% is therefore a yellow warning in the prompt's own Weather example (`before-weather-main-light.png`, last tile).
- `ComparisonCard`: green border, green tinted background, green "Recommended" pill, yellow rating pills, green pros, red cons.
- `NowPlayingCard`: a green "Playing" pill, a yellow "Paused" pill.
- `GlowBadge` and `MiniChart` accept `purple`; the prompt advertises it.
- `FileListCard`: a yellow folder; `SystemStatusCard`: a blue drive; `TaskSummaryCard`: a purple list. Three tinted glyphs for three cards, each a different hue.

HIG, widgets: "Color enhances but doesn't compete with content. Convey meaning via text and iconography, not color alone." The spec keeps one accent (system blue), one warning (system orange, set by the backend, never derived from a threshold in the component), one destructive (system red), and the done check.

### S5. Motion that does not explain anything

Every card and most children run `juno-fade-in-up` for 0.5s with per-item stagger up to 0.5s, then `juno-fill-width` 1s, bars `juno-grow-up` 0.8s staggered 80ms, numbers count for 1.2s. A four-row task card takes about 1.4s to finish arriving, on a surface where the Island's whole shape transition is 300ms and its content crossfade is 180ms. `PulseRing`, `GlowBadge`, `ShimmerText`, the sun and the rain run forever. The Island's linger ring starts counting the moment the answer finishes, so an infinite animation sits beside a timer that is trying to put the card away.

Spec: one entry per card (opacity 0 to 1 and 4px rise, 180ms, ease-out), at most three staggered rows at 40ms, meters fill in 300ms, numbers do not count, nothing loops except the `NowPlayingCard` meter moving with real playback. Reduce Motion drops the rise and the fills.

### S6. The skeleton and the fallback are not designed

- `ComponentSkeleton` is one `Skeleton` block, `h-24` to `h-64`, in `bg-muted`: oklch 0.97 on a 0.99 background in light mode, which is a faint grey rectangle nobody reads as "a card is coming" (`before-pipeline-main-light.png`, first tile). On the Island it is a 160px slate slab.
- `ComponentFallback` says "This card couldn't be shown." for every malformed close, including the task card whose labels it could have kept (`stripJsxMarkup` of an unclosed tag yields nothing).
- An unknown tag that closes (`<SwatchPicker ... />`, which the prompt teaches at `templates.rs:938-946` and nothing registers) renders as nothing at all: no skeleton, no fallback, no words. The person asked for colours and got a blank line.

Spec: a skeleton that is a card (hairline, header line, two rows, 1.2s opacity pulse) sized from the registry map; a fallback that keeps the words and names the card ("Organized Downloads: 23 images moved, 45 documents sorted"); an unknown tag becomes its words as prose.

### S7. Theme tokens are not Mac tokens

`src/styles/shadcn-theme.css`: `--font-sans: Inter, sans-serif` (Inter is not loaded by `index.html`, so the fallback is whatever `sans-serif` resolves to, not SF); `--primary: oklch(0.45 0.08 260)` is a desaturated slate, which is why `NowPlayingCard`'s play button and the "Default" button variant are navy, not system blue; `--radius: 0.625rem`. The three Mac cards get around it with a literal `#0a84ff`. The Island draws its own `rgba(255,255,255,0.55)` scale and ignores the tokens.

Spec: `--font-sans: -apple-system, "SF Pro Text", system-ui`; `--accent: #007AFF` light, `#0A84FF` dark, used by every component through one token; label scale `--label`, `--label-2` (60%), `--label-3` (30%); fills `--fill-2/3/4` (16/12/8% of `rgb(120,120,128)`); hairline `--hair` (12% black, 10% white). The Island maps the same tokens onto black.

### S8. Prompt and registry disagree

| Prompt (`templates.rs`) | Registry / component | Effect |
|---|---|---|
| `<SwatchPicker colors=...>` (938, 946) | not registered | renders nothing |
| `AnimatedNumber duration` "is in MILLISECONDS; duration={2} is two ms" (912) | the component does not clamp | the prompt documents a footgun instead of the component removing it |
| `AnimatedProgress color="purple"`, `MiniChart color="purple"`, `GlowBadge color="purple"` (913, 916, 920) | all accepted | purple chrome in answers |
| `AnimatedCard glow=` (911) | accepted | a glowing border |
| `<div className="flex justify-around">` of `<Stat>` (1019-1024) | lowercase top-level wrapper dropped (S2) | stats stack; inside a card they collide (`before-system-status-main-light.png`, "23%82%45%") |
| `<div className="flex gap-2 mt-2">` of buttons (1002-1005) | same | buttons stack |
| `ActionButton command="capture_screenshot_command"` (1079) | matches `COMMANDS.SCREENSHOTS_CAPTURE_SCREENSHOT` | fine, but the other six listed commands are already covered by `OpenButton` and `NowPlayingCard` |
| "COMPOSE RATHER THAN CONFIGURE", prefer nested Tailwind over `<WeatherCard>` (1049-1099) | both paths exist | the agent hand-rolls uppercase tracked labels, coloured badges and 4xl numbers that no component review can fix |
| `<AnimatedCard>` wrapping `<MiniChart>` + `<Stat>` (1151) | `MiniChart` bars do not render (see component) | an empty chart with labels |
| `<TaskSummaryCard>` for "Complex task" and `<StatusCard>` for success (893, 896) | `StatusCard` is the 2024 demo card | two generations in one table |

## The shared primitives

Everything below is in points at 1x, SF via `-apple-system`, sentence case. Light / dark values; the Island uses the dark set on `#000`.

| Primitive | Spec |
|---|---|
| **Card** | background `--card` (`#FFFFFF` / `#1C1C1E`; Island: transparent, no border), radius 12, border `0 0 0 0.5px var(--hair)`, no shadow, no hover. Island: no radius, no border, rows run to the 12px inset. Width: the column (S1). |
| **Header row** | 36px min, padding 6 14 0 (Island 12), title 13/600 `--label` truncating, trailing meta 11/400 `--label-2` tabular, no leading glyph by default (a card is identified by its title, not a tinted icon). |
| **List row** | padding 7 0, hairline top `--sep` (`rgba(60,60,67,0.18)` / `rgba(84,84,88,0.65)`) between rows, leading glyph 16px in `--label-2`, primary 13 `--label`, secondary line 11 `--label-2`, trailing 11 `--label-2` tabular. Pending rows use `--label-2` for the primary text. No hover tint in a chat. |
| **Meter** | 4px track `--fill-3`, 2px radius, fill `--accent`; `tone="warning"` is `#FF9500` / `#FF9F0A` and is set by the agent or backend, never computed from a threshold. Fill animates width over 300ms ease-out once. Label row above it: 13 left, value 11 `--label-2` tabular right. |
| **Stat** | value 17/600 tabular `-0.01em`, label 11 `--label-2`, left-aligned, 24px between stats, trend as "↑ Fri" in `--label-2` (no red or green). Hero variant 28/600 `-0.02em` for one number per card (temperature). |
| **Button** | 24px tall, 12/500, radius 6, padding 0 11. Secondary: `--fill-2` background, `--label` text. Primary: `--accent` background, white text. No icons. One primary per card. Buttons live in a card footer (`padding 2 14 12`, right-aligned, 8px gap) or in an `<Actions>` row under prose. Pressed: 0.85 opacity; busy: label stays, 12px spinner replaces nothing (the button is not a status). |
| **Skeleton** | a Card with a 38% header line, a 12% meta line, three body lines at 72/58/64%, `--fill-4` bars 10px tall, 1.2s opacity pulse 1 to 0.6, height from `CARD_HEIGHTS`. |
| **Access row** | sentence 13 left, one primary button right (this is `PermissionAskCard` and the `needsAccess` state of `AgendaCard` and `MessageCard`, which already match; make it one component). |
| **Motion** | card entry 180ms opacity + 4px rise, ease-out; rows stagger 40ms for the first three; meter fill 300ms; nothing infinite; `prefers-reduced-motion` removes rise and fills. |
| **Type scale** | 28 hero, 17 stat, 13 body and title, 12.5 list secondary, 12 button, 11 caption, 10 chart axis. Nothing smaller (HIG minimum 11 for widgets; 10 only for axis labels). |

Mockups: `mockup-main-light.png`, `mockup-main-dark.png`, `mockup-island.png`, rendered from `mockup.html` in the same folder (static HTML and CSS, the token values above as custom properties).

## Per-component audit

Format: what it is for; when the agent uses it (per `templates.rs`); evidence; problems quoted to the element; verdict (SHIP, CUT, REDO, MERGE); spec.

### TaskSummaryCard (the task card)

What: a checklist of what a long task did, with how much of it got done. When: the prompt's "Complex task" row and the Task Completion example; the card after "Done." Evidence: `before-task-card-main-light.png`, `before-task-card-main-dark.png`, `before-task-card-island.png`.

Problems:
- `ListTodo className="h-4 w-4 text-purple-500"` (index.tsx:598) and `bg-purple-500` meter (612) and `ArrowRight text-purple-500` (637): purple chrome on a system-blue product.
- Done rows get `line-through` and `text-muted-foreground` (630, 640): the card crosses out the work it just finished, which reads as cancelled. Reminders and Things show a filled check and keep the text.
- "2/3 done" (602): a slash fraction in a header, 11px.
- The meter turns `bg-green-500` at 100% (612): the colour changes meaning at the end.
- Width hugs the longest label (S1): 240px for the example, 700px for a long one; on the Island a 36px header and 16px card padding inside `px-2` leaves 20px of black on each side of a grey card, so the "edge to edge" face has a card inside it.
- Four stagger delays and a 1s fill before the card is readable (S5).
- Actions are outside the card, stacked (S2).
- Empty: "Nothing to do 0/0 done".

Verdict: **REDO** (first in line).

Spec: Header row "Organized Downloads" with meta "2 of 3" (or "Done" when all done, no meter). Meter (accent, 4px) under the header only while something is pending. Rows: done = 16px filled accent circle with a white check, text in `--label`; pending = 16px hairline circle, text in `--label-2`; `detail` on a second line 11 `--label-2`. Footer with the actions the agent passes (`actions={[{open: "~/Downloads", label: "Open Downloads", primary: true}, {query: "...", label: "Clean duplicates"}]}`), at most one primary. Empty: one line "Nothing to do." in `--label-2`, no header. Overflow: the title truncates, labels wrap, details wrap. Island: same, no card border, rows to the 12px inset, footer buttons inside the face. Mockup: `mockup-main-light.png` (first card), `mockup-island.png` (first island).

### WeatherCard

What: current conditions and an optional five-day row. When: "Informational" in the channel table, but the prompt then tells the agent to compose it by hand instead (1049-1099). Evidence: `before-weather-main-light.png`, `before-weather-island.png`.

Problems:
- `bg-gradient-to-br from-amber-50 to-orange-50` and friends (index.tsx:66-77): five decorative gradients, including a purple one for storms; on the Island it is a blue gradient on black.
- `juno-weather-rain::before/after` and the sun's `juno-sun-rotate 20s linear infinite` and `juno-float` (155-168): looping decoration beside a linger timer.
- `text-xs font-medium text-muted-foreground tracking-wide uppercase` location (135): uppercase tracked labels are not Mac.
- `text-4xl font-bold` (139) with `°F` at `text-lg font-normal` beside it: two type sizes for one number.
- `H: 62° L: 49°` with a thermometer glyph, `82%` with a droplet, `12 mph` with wind (179-198): three glyphs for three numbers; the numbers need words, not icons.
- Forecast is `grid-cols-3 sm:grid-cols-5` (204): at 360 the five days wrap to 3 + 2 (`before-weather-island.png`, second tile).
- `hover:shadow-md` (127): a card that lifts on hover in a chat.
- Empty state (`<WeatherCard />`) renders "CURRENT LOCATION / Clear" with a sun: a confident lie.
- The composed version the prompt prefers has an `outline` badge "Rain", a 3xl number, and a yellow 82% humidity bar.

Verdict: **REDO**.

Spec: Header row: location, meta "Updated 3:41 PM" (if the backend gives a time). Body: hero number `54°` (28/600), condition word 13 `--label-2` beside it, one 28px monochrome SF-style glyph in `--label-2` on the right. Caption line 11 tabular: "H 62°  L 49°  ·  Humidity 82%  ·  Wind 12 mph", only the parts given. Forecast: a hairline above, five fixed columns at any width (`grid-template-columns: repeat(5, 1fr)`), day 11 `--label-2`, "84 / 61" 11/500 tabular. No background, no gradient, no loop. Missing `temperature`: render nothing but the location and condition (no fake sun). Prompt: emit the tag self-closing; it is small and the skeleton covers the wait. Mockup: `mockup-main-light.png` (second card), `mockup-island.png` (second island).

### FileListCard

What: a folder's contents or a result set. When: "Data display" row. Evidence: `before-file-list-main-light.png`.

Problems:
- `Folder className="h-4 w-4 text-yellow-500"` (269): a yellow glyph as the card's identity.
- `(23)` in parentheses after a folder name (303) vs `212 KB` on the right for a file: counts and sizes in two places.
- `hover:bg-muted/50` rows (295) in a chat; nothing happens on click.
- `files.slice(0, 10)` then "+4 more" (316-320) as dead text: no way to see the rest.
- `font-mono` path at 11px (280), `px-1`: fine idea, odd inset.
- Width hugs content (S1): the populated example is 250px wide, the overflow one 670px.
- Empty: "Downloads 0 items" and the path, with no line saying it is empty.

Verdict: **REDO** on the list row primitive (small).

Spec: Header "Downloads" with meta "127 items · 4.2 GB". Path as an 11px mono caption, truncating from the left (`direction: rtl` on an inline block) so the leaf stays visible. Rows: 16px `--label-2` glyph (folder or file type), name 13 truncating, trailing 11 `--label-2` tabular: "23 items" for folders, size for files, one column. After eight rows: a last row "Show all 127" in accent 12/500 that is a `QueryButton` ("list all files in ~/Downloads"). Empty: header plus one line "Empty." in `--label-2`. Mockup: `mockup-main-light.png` (third card).

### SystemStatusCard

What: a few named percentages. When: "Data display" row, but the prompt prefers the composed version with `AnimatedProgress` and a `Stat` row. Evidence: `before-system-status-main-light.png`.

Problems:
- `HardDrive className="text-blue-500"` (390).
- `MetricBar` colour by threshold (350-355): green, yellow, red decided by the component.
- "MacBook ProUp: 3d 12h" (391-396): the header meta has no gap because the card is `w-fit` and `justify-between` has nothing to spread.
- The composed version's Stat row renders "23%82%45%" (prompt 1019-1024, S2).
- Empty: a drive glyph and "System Status".

Verdict: **MERGE** into a `MetricList` built from the meter primitive (`SystemStatusCard` becomes a thin alias for a prompt cycle, then goes).

Spec: Header "MacBook Pro", meta "Up 3d 12h". Rows: label 13, value 11 `--label-2` tabular right ("23%", "13.1 of 16 GB"), 4px accent meter; `tone="warning"` from the agent turns the one that matters orange and its value text orange with a word ("94%, almost full"). No stat row duplicate underneath. Mockup: `mockup-main-light.png` (fourth card).

### ComparisonCard

What: two or three options with pros, cons, a rating and a pick. When: "Research/comparison" row. Evidence: `before-comparison-main-light.png`, `before-comparison-island.png`.

Problems:
- Recommended option: `border-green-500 bg-green-50/50 shadow-green-500/10` (449) and a green pill (456).
- Rating as five `bg-yellow-400` pills (468).
- Pros in `text-green-700` with a check (485), cons in `text-red-600` with a circle (499): colour does the work the + and − words should.
- `grid-cols-3` at any width (439): three 100px columns at 360, names wrapping to two lines and the "Recommended" pill drawn over the next column's name (`before-comparison-island.png`, second tile).
- `hover:shadow-sm` per option (447).
- `options="not an array"` throws, caught by the boundary, becomes "This card couldn't be shown." with the title lost.

Verdict: **REDO**.

Spec: Header "React vs Vue". Options as a 2-column grid at 480px and up, one column below (the Island, the Pill): each option a row group with name 13/600 and "Recommended" 11/500 in accent beside it, five 6px dots (accent filled, `--fill-2` empty) for the rating, then pros and cons as 12.5 lines with a `--label-2` "+" or "−" in the gutter. No borders between options in one column; a hairline at 2 columns. Mockup: `mockup-main-light.png` (seventh card).

### LinkCard

What: a page the agent found. When: a result the person may open. Evidence: `before-link-card-main-light.png`.

Problems:
- The whole card is `hover:bg-muted/50 hover:shadow-sm` (546) but nothing is clickable: a hover state on a dead card.
- `favicon` is an arbitrary `img src` (551): a remote fetch from an agent-provided URL in the chat window.
- Domain shown twice when there is no title (557, 565).

Verdict: **REDO** (small), merging the open action in.

Spec: a row card: title 13/600 truncating, "domain · description" 11 `--label-2` on one truncating line, a secondary "Open" button on the right that is an `OpenButton url=`. No favicon (or only a domain initial in a `--fill-3` circle). Empty (`<LinkCard />`): nothing. Mockup: `mockup-main-light.png` (eighth card).

### ImageCard

What: a PNG Juno generated or edited, with Open. When: after `generate_image` or `edit_image`. Evidence: `before-image-card-main-light.png`.

Problems:
- `hover:shadow-md` (685).
- The "Open" button carries an external-link glyph; it opens Preview.
- `before`/`after` panes have no labels (690-691); `alt="before"` is the only hint.
- `openInPreview` builds `file://` + `path.replace(/^~\//, "")`: `~/Pictures/x.png` becomes `file://Pictures/x.png`, the exact tilde bug `OpenButton` fixed at agent-actions.tsx:311-316. Not a design finding, but it means the one primary action does not work for the path the prompt tells the agent to use.

Verdict: **SHIP with polish** (the fix above is a bug, not a redesign).

Spec: no hover; caption 13 left, "Open" secondary button right (reuse the button primitive); before/after get 11px `--label-2` captions "Before" and "After" under each pane; max height 320 with `object-fit: contain` so a tall image does not take the Island.

### NowPlayingCard

What: the live player. When: every playback answer; also emitted by `local_intents` before the model runs. Evidence: `before-now-playing-main-light.png`, `before-now-playing-island.png`, `before-now-playing-not-running.png`.

Problems:
- "NOW PLAYING · SPOTIFY" in `uppercase tracking-wide` (224-226).
- Status pill `bg-green-100 text-green-700` / yellow (229-246): the play/pause glyph already says it.
- Primary control is `bg-primary` (202), the slate token, not system blue; on the Island it is a navy disc on black.
- `h-12 w-12` art (266) beside a title at 14px and an artist at 12px: the art is bigger than the text block and the row is 48px tall plus 12px header plus 12px times.
- Loading: "Reading Spotify…" with a spinner, in the card's full chrome.
- Error text at `text-[10px]` (314) in red under the row: "Error: AppleScript error: Spotify got an error: Application isn't running. (-600)" is shown to the person, under a line that already says "Spotify isn't running." (`before-now-playing-error.png`).
- Not running: the "Open Spotify" button is a hand-styled `h-7 px-2.5` outline, different from `OpenButton`, and a grey "Not running" pill repeats the sentence beside it (`before-now-playing-not-running.png`).
- On the Island the title truncates at 14 characters because the 48px art, the 36px disc and two 32px buttons leave 150px for text (`before-now-playing-island.png`).

Verdict: **REDO the chrome, SHIP the behaviour** (polling, optimistic flip, artwork export are right).

Spec: Header "Spotify" (or "Apple Music"), meta "Playing" / "Paused" / "Not running" in `--label-2`. Row: 44px art radius 6 (or a `--fill-3` square with a note glyph), title 13/600 truncating, "artist · album" 11 `--label-2` truncating, 3px accent meter that moves with playback, times 10 tabular. Controls: three 28px glyph buttons in `--label` (previous, play/pause at 32px, next), no disc. Loading: the skeleton. Not running: one line "Spotify isn't open." and a secondary "Open Spotify" button (the button primitive). Error: the card keeps the last state and the meta reads "Couldn't reach Spotify" in `--label-2`; the raw error goes to the log, per `juno-fail-silently`. Mockup: `mockup-main-light.png` (sixth card), `mockup-island.png` (third island).

### AgendaCard, MessageCard, PermissionAskCard

What: events and reminders; a text or email as it goes out; "I need Reminders for that." When: emitted finished by the Mac app tools and `local_intents`. Evidence: `before-agenda-main-light.png`, `before-message-main-light.png`.

These three are the bar. Hairline rows, `#0a84ff` once, no gradient, no entry animation, designed empty and access states, sentence case. Problems are small:
- `AgendaCard` done rows get `line-through` (agenda-card.tsx:142): same objection as the task card; the filled blue check is enough.
- `MessageCard` state line "Say "send it"" (67) is the right idea; "Sending" with a spinner and "Sent" with a check are right.
- `PermissionAskCard` from markup never shows its button (`onContinue` cannot cross the JSX boundary), so the agent-emitted version is a sentence in a card with no action. The approval prompt path is the one that works (and #815 is extending it). Decide which path owns it; today the registered tag is a dead end.
- The `needsAccess` branch is duplicated in two files (agenda-card.tsx:93-118, message-card.tsx:103-128) and is the same shape as `PermissionAskCard`.
- Hard-coded `#0a84ff` instead of a token: fix with S7.

Verdict: **SHIP**, with the access row extracted (`AccessRow`, one file, three callers) and `line-through` removed. These are the reference implementation for the rest of the set.

### WhyBlock

What: collapsed method rationale. Evidence: `before-why-main-light.png`.

Problems: a lightbulb glyph plus "Why I did it this way" plus a chevron, 14px, is three affordances for a disclosure; the lightbulb reads as a tip. Verdict: **SHIP with polish**: chevron and label only, 12px `--label-2`, chevron leading like a macOS disclosure triangle.

### Stat and AnimatedNumber

Evidence: `before-stat-main-light.png`, `before-animated-number-main-light.png`.

Problems: `text-center` (468) so a row of stats centres each in its own column and stacks when the wrapper is dropped (S2); `text-2xl font-bold` (469) is 24px bold, louder than any Mac widget's secondary number; trend arrows in `text-green-500` / `text-red-500` (456-457) turn "82% RAM, up" green; `AnimatedNumber` counts for 1.2s by default and is a separate tag from `Stat`, so the prompt tells the agent to use both.

Verdict: **MERGE** `AnimatedNumber` into `Stat` (and cut the count-up); `Stat` is a primitive, not a tag the agent composes rows of.

Spec: left-aligned, 17/600 tabular, label 11 `--label-2`, trend text in `--label-2`. A `<Stats>` container lays them out in a row with 24px gaps and wraps at 360. `hero` variant for one number per card. Mockup: `mockup-main-light.png` (fifth card, bottom row).

### MiniChart

Evidence: `before-minichart-main-light.png`.

Problems: **the bars do not render.** Each bar sets `height: ${barHeight}%` inside a `flex-1 flex flex-col items-center` column (526-535) that has no height of its own, so every bar collapses to `min-h-[2px]`: a row of 2px dashes under the labels, on every surface. The prompt's "Data with Chart" example (1031-1046) ships an empty chart. Labels at `text-[10px]` truncate to "M…", "W…" at 7 bars in a `w-fit` card. `color="purple"` and `orange` are accepted. Opacity varies with value (533), so short bars are also paler.

Verdict: **REDO** (and it is a bug today).

Spec: bars are the flex children themselves (no inner column), `height` in px from the container height, accent at full opacity, 2px top radius, 3px gap, 56px tall; labels 10 `--label-2` with `white-space: nowrap` and no truncation (hide every other label when there are more than 8). Optional `highlight` index for the bar the answer is about; others in `--fill-2`. Mockup: `mockup-main-light.png` (fifth card).

### AnimatedProgress and ProgressBar

Evidence: `before-animated-progress-main-light.png`, `before-showcase-main-light.png`.

Problems: two progress bars in the registry (`ProgressBar` from 2024 with `bg-gray-200` and a "75%" line under it; `AnimatedProgress` with threshold colours, `purple`, and `h-2`). `value={140}` clamps to 100 but shows "100%" in red. The bare bar is 0px wide (S1).

Verdict: **MERGE** into `Meter` (the primitive). Cut `ProgressBar`. Cut `color`; add `tone="warning"`.

### AnimatedCard and AnimatedList

Evidence: `before-animated-card-main-light.png`.

Problems: `glow` paints a coloured `boxShadow` and border (53-57); four entry animations; `hover:shadow-md`; an empty `<AnimatedCard>` renders as a 32px rounded blob. `AnimatedList` wraps each child in its own staggered div (156-163) and nothing else.

Verdict: **MERGE** both into `Card` (one entry, no glow, no hover). The prompt's composed examples become `<Card title="Weekly activity" meta="This week">` with children. `AnimatedList` goes; the row stagger lives in the list row primitive.

### GlowBadge, ShimmerText, PulseRing, Confetti, AnimatedDivider

Evidence: `before-effects-main-light.png`.

Problems: glowing pills in five colours including purple (`juno-glow` infinite); a shimmer that is a `rgba(255,255,255,0.1)` sweep, invisible in light mode; `PulseRing` loops forever beside a linger timer; `Confetti` fires once for 0.8s and leaves a 32px empty box (the screenshot at 2s shows nothing); `AnimatedDivider` renders a 1px gradient that is invisible on both themes (second tile is blank). The prompt already says to reach for them "rarely".

Verdict: **CUT** all five, and the `EFFECTS` section of the prompt (919-924). `Separator` covers the divider.

### Circle, Rectangle, Triangle, ColorShowcase, StatusCard, ProgressBar, VisualDemo

Evidence: `before-showcase-main-light.png`.

Problems: shapes with `2px solid black` borders and a caption that recites their own props ("Triangle: 60px, orange, pointing up"); `StatusCard` tints a whole row green, yellow, red or blue with `text-green-800` copy; `VisualDemo` is a self-advertisement ("Examples of what I can create instead of typing raw code"). The channel table still lists `<StatusCard>` for success confirmations.

Verdict: **CUT** all seven. Success confirmations are a sentence (the prompt already says so in the same row). If the agent needs a callout, `Alert` remains.

### OpenButton, QueryButton, CopyButton, ActionButton

Evidence: `before-buttons-main-light.png`, `before-buttons-island.png`.

Problems: four files' worth of the same `variantClasses` / `sizeClasses` tables (agent-actions.tsx:150-161, 236-247, 333-344, 406-417); every button carries a 12px glyph (folder, external link, speech bubble, copy, terminal) so a row of three buttons is a row of three icons; "Done" and "Copied!" replace the label on success, so the button changes width; `QueryButton` without a label shows the whole query as the button; the `default` variant is the slate `--primary`; `ActionButton` has a seven-command allowlist of which `open_url` and `open_application` are `OpenButton`, `media_control` is `NowPlayingCard`, and the rest route to `QueryButton` anyway (133-137). They stack when the prompt's `<div>` wrapper is dropped (S2).

Verdict: **REDO** on the button primitive; **CUT** `ActionButton`; add `<Actions>`.

Spec: one `Button` base (24px, 12/500, radius 6, secondary fill or primary accent, no icon). `OpenButton`, `QueryButton`, `CopyButton` are three thin wrappers. `primary` prop, at most one per row. Success: `CopyButton` shows "Copied" for 1.5s at the same width (`min-width` from the label); `QueryButton` shows nothing (the new user message appearing is the feedback); `OpenButton` shows nothing. Error: a 11px `--label-2` line under the row, one sentence. `<Actions>` is a capitalised row container (flex, wrap, 8px gap, right-aligned in a card footer, left-aligned under prose) so the splitter keeps the group. Mockup: `mockup-main-light.png` (task card footer).

### Card, Alert, Badge, Tabs, Separator, Button, Input, Textarea, Switch, Select, Dialog, Tooltip, Label, Skeleton (shadcn passthrough)

Evidence: `before-passthrough-main-light.png`.

Problems: `Input`, `Textarea`, `Switch`, `Select` are controls bound to nothing, the exact "fake state" the prompt's hard rule forbids (1101-1111); `Dialog` and `Tooltip` need triggers and portals the chat does not host; `Skeleton` is the renderer's own loading state; bare `Button` has no `onClick` path, so it is a button that does nothing; `Badge` in four variants is where "Rain" pills come from; `Tabs` hide half an answer behind a click, which is the opposite of glanceable.

Verdict: **CUT** `Input`, `Textarea`, `Switch`, `Select*`, `Dialog*`, `Tooltip*`, `Label`, `Skeleton`, bare `Button`, `Tabs*`. **SHIP** `Card*` (restyled to the primitive), `Alert*` (one neutral variant with `--fill-4` background and no colour), `Badge` restricted to `outline` and `secondary`, `Separator`.

### Icons (36 lucide names)

Verdict: **SHIP** but render them in `--label-2` at 14px by default and strip `text-{color}` classes the agent passes (one `cn` filter in the renderer), so icons cannot become chrome. Keep the list; the prompt's "Icons" line stays.

### div, span, p, h1 to h6

Verdict: **SHIP**, with the renderer dropping `className` values that set colour (`text-*-500`, `bg-*-50`, gradients) and letting layout and spacing classes through. This is the backstop for the prompt change in P5.

### ComponentSkeleton, ComponentFallback, the streaming cursor

Evidence: `before-pipeline-main-light.png`.

Verdict: **REDO** per S6. The streaming cursor (`mixed-content-renderer.tsx:351`, a pulsing `|` in a 2x4 block) only shows when there are two or more segments; make it one 2px wide bar at the end of the last text segment, at `--label-2`, and never after a card.

## Cut and merge table

| Tag | Verdict | Goes to |
|---|---|---|
| GlowBadge, ShimmerText, PulseRing, Confetti, AnimatedDivider | CUT | `Separator` for the divider |
| Circle, Rectangle, Triangle, ColorShowcase, VisualDemo, StatusCard, ProgressBar | CUT | `Alert` (neutral), `Meter` |
| ActionButton | CUT | `OpenButton`, `QueryButton`, `NowPlayingCard` |
| Input, Textarea, Switch, Select*, Dialog*, Tooltip*, Label, Skeleton, Button, Tabs* | CUT from the registry | |
| AnimatedCard, AnimatedList | MERGE | `Card` |
| AnimatedNumber | MERGE | `Stat` |
| AnimatedProgress | MERGE | `Meter` |
| SystemStatusCard | MERGE | `MetricList` (meter rows) |
| the `needsAccess` branches of AgendaCard and MessageCard, PermissionAskCard | MERGE | `AccessRow` |
| TaskSummaryCard, WeatherCard, FileListCard, ComparisonCard, LinkCard, NowPlayingCard (chrome), MiniChart, Stat, buttons, skeleton, fallback | REDO | the primitives |
| AgendaCard, MessageCard, PermissionAskCard, WhyBlock, ImageCard, Card*, Alert*, Badge (2 variants), Separator, icons, html tags | SHIP | with the polish noted |

New tags: `Actions`, `Stats`, `Meter`, `MetricList`, `AccessRow`. Net: 38 custom tags become 17.

## Prompt changes (`src-tauri/src/agent/prompts/templates.rs`)

1. Delete the `EFFECTS` block (919-924) and the `Shapes` line (905); delete `StatusCard` and `ProgressBar` from `Data Display` (904) and `<StatusCard>` from the channel table (896).
2. Delete `Interactive: Button, Input, Textarea, Select, Switch, Label, Dialog` (905). Replace with `Actions: OpenButton, QueryButton, CopyButton inside <Actions>`.
3. Replace the `ANIMATED COMPONENTS` block (909-917) with `PRIMITIVES`: `<Card title meta>`, `<Meter value label tone?>`, `<Stats><Stat value label trend?/></Stats>`, `<MiniChart data labels highlight?>`. Remove `glow`, `duration`, every `color=` prop, and the millisecond footnote (912).
4. Rewrite `STREAMING COMPONENT COMPOSITION` (1047-1099): keep "emit the container first", drop "COMPOSE RATHER THAN CONFIGURE" and the two hand-rolled weather and system examples. New rule: use the domain card when one exists; compose only inside `<Card>` with the primitives; never set colour classes, uppercase or tracking; the skeleton covers the wait.
5. Rewrite the four `RESPONSE FORMAT EXAMPLES` (983-1046) on the new tags, with buttons inside `<Actions>` or the card's `actions` prop and no top-level `<div>`.
6. Remove `SwatchPicker` (936-946) or register a `SwatchRow` (five 24px circles with 11px names, tap = `QueryButton`); today it renders nothing.
7. `IMPORTANT, ActionButton vs QueryButton` (1077-1083): delete; `ActionButton` is gone.
8. Rule 11 (1151): "Compose animated primitives inside one card" becomes "One card per answer unless the person asked for two things."
9. Add the `tone` rule: a meter or a value is orange only when the agent says `tone="warning"` because the number is a problem for this person, never because it is over 70.

## Implementation order

| # | Work | Size | Unblocks |
|---|---|---|---|
| 1 | S1 `w-full` on assistant `MessageContent`; S2 `<Actions>` tag and splitter keeps a lowercase wrapper that contains a capitalised tag | S | every card's width, every button row |
| 2 | Tokens (S7): font, accent, label and fill scale, hairline; Island maps them on black | S | everything below |
| 3 | Primitives: `Card`, header row, list row, `Meter`, `Stat` + `Stats`, `Button` base, `Skeleton`, `AccessRow` (extract from the three Mac cards) | M | 4 to 7 |
| 4 | Task card redo on the primitives, with `actions` and the footer | S | the demo |
| 5 | Registry cut (table above) and prompt changes 1, 2, 7 | S | fewer wrong answers immediately |
| 6 | `NowPlayingCard` chrome, `MiniChart` fix, `AnimatedProgress` to `Meter`, buttons to the base | M | |
| 7 | `WeatherCard`, `FileListCard`, `ComparisonCard`, `LinkCard`, `SystemStatusCard` to `MetricList` | M | |
| 8 | Prompt changes 3 to 6, 8, 9; re-record the gallery for every surface | S | |
| 9 | Skeleton and fallback redo (S6), renderer colour-class filter | S | |

Sizes: S under half a day, M one to two days. 1 through 5 is the smallest scope that passes the demo test on the task card; ship that first.

## Demo test

1. Ten seconds: "Organize my downloads." Juno says "Done." A card slides up 4px: "Organized Downloads, 3 of 3", three blue checks, and "Open Downloads" in blue at the bottom right. One tap opens Finder.
2. Removed: 21 tags, five infinite animations, every gradient, every threshold colour, the strikethrough, the icon tint, the hover lift, the count-up, the per-button icon, the stagger past three rows.
3. One primary action per card: the `primary` flag, enforced by `Actions`.
4. Defaults: accent and neutral; `tone` only when the agent says so.
5. Empty, loading, error states: specified per card above; skeleton and fallback redone.
6. Instant: no card takes longer than 300ms to be readable.
7. Seams closed: the Island draws the same card with no inner border; the Pill and the main window get the same width behaviour.
8. Stage test: the task card mockup, yes. The current one, no.
9. Evidence: `docs/changelog/media/822/`.
10. DRI: the engineer who picks up step 1; Lacy approves the primitive set before step 3.

## Running the gallery

```
bunx vite --port 1453
open "http://localhost:1453/__component-gallery?surface=main&theme=light"
open "http://localhost:1453/__component-gallery?surface=island&c=TaskSummaryCard"
open "http://localhost:1453/__component-gallery?surface=main&c=NowPlayingCard&media=not_running"
```

Vite ignores `.claude/**`, so restart it after editing the gallery in a worktree.
