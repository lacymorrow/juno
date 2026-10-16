/**
 * Gallery of every agent-rendered component (route `/__component-gallery`).
 *
 * A dev-only bench for the component redesign audit
 * (docs/plans/component-redesign-audit.md). It runs the REAL pipeline, the
 * markup the agent would emit through `MixedContentRenderer`, so skeletons,
 * partial tags, fallbacks and the error boundary all behave as they do in a
 * conversation. The Tauri layer is the same in-memory stand-in the bar bench
 * uses, extended with the two media commands the live card reads, so nothing
 * here can reach a real player or open a real file.
 *
 * Query:
 *   surface=main|pill|island   which chrome to draw around the component
 *   theme=light|dark           main only; the pill and the island are dark
 *   c=<ComponentName>          one component (default: all)
 *   state=<key>                one state of that component (default: all)
 *   media=playing|paused|stopped|not_running|loading|error
 *                              what `media_get_state` answers (NowPlayingCard)
 *
 * Unlinked, never reachable through normal UI. Only this route imports it.
 */

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { useMemo } from "react";
import { COMMANDS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";
import { Message, MessageContent } from "@/components/ai-elements/message";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";

// ── The Tauri stand-in ───────────────────────────────────────────────

type MediaMood = "playing" | "paused" | "stopped" | "not_running" | "loading" | "error";

function mediaFor(mood: MediaMood, app: string) {
  if (mood === "not_running") {
    return { app, running: false, state: "not_running", track: null, artist: null, album: null, position_secs: null, duration_secs: null, artwork_url: null };
  }
  if (mood === "stopped") {
    return { app, running: true, state: "stopped", track: null, artist: null, album: null, position_secs: null, duration_secs: null, artwork_url: null };
  }
  return {
    app,
    running: true,
    state: mood,
    track: "Midnight City (Extended Night Drive Edition)",
    artist: "M83",
    album: "Hurry Up, We're Dreaming",
    position_secs: 102,
    duration_secs: 255,
    artwork_url: ART,
  };
}

/** A square of "album art" so no network is needed. Purple inside artwork is fine. */
const ART =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#5b3fa6"/><stop offset="1" stop-color="#e2574c"/></linearGradient></defs><rect width="96" height="96" fill="url(#g)"/><circle cx="48" cy="48" r="22" fill="none" stroke="#fff" stroke-opacity=".5" stroke-width="6"/></svg>`,
  );

function picture(label: string, hue: number) {
  return (
    "data:image/svg+xml;utf8," +
    encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400"><rect width="640" height="400" fill="hsl(${hue},28%,62%)"/><rect x="40" y="260" width="560" height="100" fill="hsl(${hue},20%,40%)"/><rect x="90" y="120" width="140" height="140" rx="8" fill="hsl(${hue},18%,80%)"/><text x="320" y="60" font-family="-apple-system,Helvetica" font-size="22" text-anchor="middle" fill="#fff" opacity=".8">${label}</text></svg>`,
    )
  );
}

let installed = false;
function installGalleryTauri(mood: MediaMood): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  mockWindows("main");
  mockIPC(
    async (cmd, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case COMMANDS.MEDIA_GET_STATE: {
          if (mood === "loading") return new Promise(() => {});
          if (mood === "error") throw new Error("AppleScript error: Spotify got an error: Application isn't running. (-600)");
          return mediaFor(mood, String(args.app ?? "Spotify"));
        }
        case COMMANDS.MEDIA_CONTROL:
          return mediaFor(args.action === "pause" ? "paused" : "playing", String(args.app ?? "Spotify"));
        case "plugin:path|resolve_directory":
          return "/Users/demo/";
        case "plugin:window|theme":
          return null;
        default:
          // open_url, open_application, dispatch_query, clipboard: accepted and dropped.
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  // `convertFileSrc` is not part of the IPC mock; the ImageCard needs it.
  const internals = (window as unknown as { __TAURI_INTERNALS__: Record<string, unknown> }).__TAURI_INTERNALS__;
  internals.convertFileSrc = (path: string) =>
    /before|original/.test(path) ? picture("before", 210) : picture("after", 120);
}

// ── The catalogue: every registered component, in its real states ────

interface GalleryState {
  key: string;
  note?: string;
  jsx: string;
  /** Render as if the stream is still open. */
  streaming?: boolean;
}

interface GalleryEntry {
  name: string;
  group: string;
  states: GalleryState[];
}

const LONG =
  "A very long label that keeps going well past the width of any card so truncation and wrapping can be judged honestly";

const CATALOGUE: GalleryEntry[] = [
  {
    name: "TaskSummaryCard",
    group: "Domain cards",
    states: [
      { key: "populated", jsx: `<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved to Images/", done: true}, {label: "45 documents sorted", done: true, detail: "PDF, DOCX, TXT"}, {label: "15 videos categorized", done: false}]} />` },
      { key: "all-done", jsx: `<TaskSummaryCard title="Cleanup Results" tasks={[{label: "Deleted temp files", done: true}, {label: "Emptied Trash", done: true}]} />` },
      { key: "empty", jsx: `<TaskSummaryCard title="Nothing to do" tasks={[]} />` },
      { key: "overflow", jsx: `<TaskSummaryCard title="${LONG}" tasks={[{label: "${LONG}", done: true}, {label: "Short one", done: false, detail: "${LONG}"}]} />` },
      { key: "streaming-tag", note: "open tag still arriving: skeleton", streaming: true, jsx: `<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved", done: true}, {label: "45 docu` },
      { key: "fallback", note: "known component, never closed, stream ended", jsx: `<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved", done: true}` },
      { key: "with-actions", note: "footer actions; the prompt's Task Completion example", jsx: `<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved to Images/", done: true}, {label: "45 documents sorted", done: true, detail: "PDF, DOCX, TXT"}, {label: "15 videos categorized", done: false}]} actions={[{open: "~/Downloads", label: "Open Downloads", primary: true}, {query: "Delete duplicate files in Downloads", label: "Clean duplicates"}]} />` },
      { key: "actions-row", note: "buttons in a lowercase div under prose: one block, one row", jsx: `Done. I sorted everything into five folders.\n\n<div className="flex gap-2 mt-2">\n  <OpenButton path="~/Downloads" label="Open Downloads" primary />\n  <QueryButton query="Delete duplicate files in Downloads" label="Clean duplicates" />\n</div>` },
      { key: "composed-card", note: "a composed shadcn Card", jsx: `<Card><CardHeader><CardTitle>Storage</CardTitle><CardAction className="text-[11px] text-muted-foreground">Macintosh HD</CardAction></CardHeader><CardContent><AnimatedProgress value={62} label="Used" /><Stats className="mt-3"><Stat value="312 GB" label="Used" /><Stat value="188 GB" label="Free" /></Stats></CardContent><CardFooter><Actions><QueryButton query="Find large files" label="Find large files" primary /></Actions></CardFooter></Card>` },
    ],
  },
  {
    name: "WeatherCard",
    group: "Domain cards",
    states: [
      { key: "rain", jsx: `<WeatherCard location="San Francisco" temperature={54} unit="F" condition="rain" high={62} low={49} humidity={82} wind="12 mph" />` },
      { key: "sunny-forecast", jsx: `<WeatherCard location="Charlotte" temperature={78} unit="F" condition="sunny" high={84} low={61} humidity={40} wind="4 mph" forecast={[{day: "Mon", high: 84, low: 61, condition: "sunny"}, {day: "Tue", high: 80, low: 60, condition: "cloudy"}, {day: "Wed", high: 71, low: 58, condition: "rain"}, {day: "Thu", high: 69, low: 55, condition: "storm"}, {day: "Fri", high: 73, low: 57, condition: "clear"}]} />` },
      { key: "snow", jsx: `<WeatherCard location="Denver" temperature={28} unit="F" condition="snow" high={31} low={18} />` },
      { key: "empty", note: "no props at all", jsx: `<WeatherCard />` },
      { key: "forecast-and-time", note: "forecast at any width, with an update time", jsx: `<WeatherCard location="Charlotte" temperature={78} unit="F" condition="partly cloudy" high={84} low={61} humidity={40} wind="4 mph" updated="3:41 PM" forecast={[{day: "Mon", high: 84, low: 61, condition: "sunny"}, {day: "Tue", high: 80, low: 60, condition: "cloudy"}, {day: "Wed", high: 71, low: 58, condition: "rain"}, {day: "Thu", high: 69, low: 55, condition: "storm"}, {day: "Fri", high: 73, low: 57, condition: "clear"}]} />` },
      { key: "no-temperature", note: "only the location and the condition: no invented numbers", jsx: `<WeatherCard location="Boone" condition="snow" />` },
      { key: "streaming-tag", note: "open tag still arriving: skeleton", streaming: true, jsx: `<WeatherCard location="San Francisco" temperature={54} condition="ra` },
    ],
  },
  {
    name: "FileListCard",
    group: "Domain cards",
    states: [
      { key: "populated", jsx: `<FileListCard title="Downloads" path="~/Downloads" totalCount={127} totalSize="4.2 GB" files={[{name: "Images", type: "folder", count: 23}, {name: "Documents", type: "folder", count: 45}, {name: "invoice-2026-10.pdf", type: "document", size: "212 KB"}, {name: "screen-recording.mov", type: "video", size: "1.8 GB"}, {name: "mix.mp3", type: "audio", size: "9 MB"}, {name: "archive.zip", type: "archive", size: "340 MB"}]} />` },
      { key: "overflow", note: "14 files, long names", jsx: `<FileListCard title="Desktop" path="~/Desktop/Projects/Client Work/2026/October/Deliverables/Final" totalCount={14} files={[{name: "${LONG}.pdf", type: "document", size: "1 KB"}, {name: "b.txt"}, {name: "c.txt"}, {name: "d.txt"}, {name: "e.txt"}, {name: "f.txt"}, {name: "g.txt"}, {name: "h.txt"}, {name: "i.txt"}, {name: "j.txt"}, {name: "k.txt"}, {name: "l.txt"}, {name: "m.txt"}, {name: "n.txt"}]} />` },
      { key: "empty", jsx: `<FileListCard title="Downloads" path="~/Downloads" totalCount={0} files={[]} />` },
    ],
  },
  {
    name: "MetricList",
    group: "Domain cards",
    states: [
      { key: "populated", note: "tone is set by the agent, never by the number", jsx: `<MetricList title="MacBook Pro" meta="Up 3d 12h" metrics={[{label: "CPU", value: 23}, {label: "Memory", value: 82, detail: "13.1 of 16 GB"}, {label: "Disk", value: 94, detail: "94%, almost full", tone: "warning"}]} />` },
      { key: "alias", note: "SystemStatusCard, the earlier tag, still renders", jsx: `<SystemStatusCard hostname="MacBook Pro" uptime="3d 12h" metrics={[{label: "CPU", value: 23}, {label: "Memory", value: 82}, {label: "Disk", value: 94}]} />` },
      { key: "empty", jsx: `<MetricList />` },
      { key: "streaming-tag", note: "open tag still arriving: skeleton", streaming: true, jsx: `<MetricList title="MacBook Pro" metrics={[{label: "CPU", val` },
    ],
  },
  {
    name: "ComparisonCard",
    group: "Domain cards",
    states: [
      { key: "two", jsx: `<ComparisonCard title="React vs Vue" options={[{name: "React", pros: ["Large ecosystem", "Hiring pool"], cons: ["Steep curve"], rating: 4, recommended: true}, {name: "Vue", pros: ["Gentle learning curve"], cons: ["Smaller ecosystem", "Fewer jobs"], rating: 3}]} />` },
      { key: "three", jsx: `<ComparisonCard title="Laptops under $1,500" options={[{name: "MacBook Air 15", pros: ["Battery", "Quiet"], cons: ["Two ports"], rating: 5, recommended: true}, {name: "ThinkPad X1", pros: ["Keyboard"], cons: ["Battery", "Price"], rating: 3}, {name: "Framework 13", pros: ["Repairable", "Upgradable"], cons: ["Fan noise"], rating: 4}]} />` },
      { key: "empty", jsx: `<ComparisonCard title="Nothing to compare" options={[]} />` },
      { key: "bad-options", note: "bad prop type: designed empty state, title kept", jsx: `<ComparisonCard title="Oops" options="not an array" />` },
      { key: "narrow", note: "side by side from 480px; the Island and Pill stay one column (use ?surface=island)", jsx: `<ComparisonCard title="Laptops under $1,500" options={[{name: "MacBook Air 15", pros: ["Battery"], cons: ["Two ports"], rating: 5, recommended: true}, {name: "ThinkPad X1", pros: ["Keyboard"], cons: ["Price"], rating: 3}]} />` },
    ],
  },
  {
    name: "LinkCard",
    group: "Domain cards",
    states: [
      { key: "populated", jsx: `<LinkCard url="https://developer.apple.com/design/human-interface-guidelines/widgets" title="Widgets | Apple Developer Documentation" description="Widgets display timely, glanceable content and offer focused interactions in additional contexts beyond your app." />` },
      { key: "bare", jsx: `<LinkCard url="https://junebug.ai" />` },
      { key: "empty", jsx: `<LinkCard />` },
    ],
  },
  {
    name: "ImageCard",
    group: "Domain cards",
    states: [
      { key: "single", jsx: `<ImageCard path="~/Pictures/Juno/walls-in-sage.png" caption="Walls in Sage #7A9E7E" />` },
      { key: "before-after", jsx: `<ImageCard path="~/Pictures/Juno/walls-in-sage.png" before="~/Pictures/Juno/original.png" caption="Before and after" />` },
      { key: "empty", note: "no path: renders nothing", jsx: `<ImageCard caption="No path" />` },
    ],
  },
  {
    name: "NowPlayingCard",
    group: "Live cards",
    states: [
      { key: "spotify", note: "state follows ?media=", jsx: `<NowPlayingCard app="Spotify" />` },
      { key: "music", jsx: `<NowPlayingCard app="Music" />` },
    ],
  },
  {
    name: "AgendaCard",
    group: "Live cards",
    states: [
      { key: "items", jsx: `<AgendaCard items={[{title: "Dentist", when: "Today 3:00 PM", where: "Park Rd", kind: "event"}, {title: "Call Maya about Friday", when: "Tomorrow 9:00 AM", kind: "reminder"}, {title: "Pay water bill", when: "Oct 20", kind: "reminder", done: true}, {title: "${LONG}", when: "Oct 21, all day", where: "${LONG}", kind: "event"}]} />` },
      { key: "empty", jsx: `<AgendaCard items={[]} empty="Nothing on the calendar today." />` },
      { key: "needs-access", jsx: `<AgendaCard needsAccess="Reminders" settingsUrl="x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders" />` },
    ],
  },
  {
    name: "MessageCard",
    group: "Live cards",
    states: [
      { key: "draft", jsx: `<MessageCard state="draft" kind="text" to="Maya Chen" address="+1 (704) 555-0142" body="Can we move Friday to 3? Lacy's dentist ran over." />` },
      { key: "sending", jsx: `<MessageCard state="sending" kind="email" to="Doug Keesler" address="doug@example.com" subject="PH launch checklist" body="Doug,\n\nThe cutover list is attached. Nothing goes live until you say so.\n\nCheers" />` },
      { key: "sent", jsx: `<MessageCard state="sent" kind="text" to="Maya Chen" body="Can we move Friday to 3?" />` },
      { key: "needs-access", jsx: `<MessageCard needsAccess="Full Disk Access" reason="I need Full Disk Access to read your messages." settingsUrl="x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles" />` },
    ],
  },
  {
    name: "PermissionAskCard",
    group: "Live cards",
    states: [
      { key: "waiting", note: "no onContinue from markup: no button", jsx: `<PermissionAskCard line="I need Reminders for that." />` },
    ],
  },
  {
    name: "Why",
    group: "Live cards",
    states: [
      { key: "collapsed", jsx: `Playing. Midnight City by M83.\n\n<NowPlayingCard app="Spotify" />\n\n<Why>\nSpotify was already running, so \`osascript\` hit its scripting dictionary directly: no screenshot, no coordinate guessing, no stolen focus from your editor.\n</Why>` },
    ],
  },
  {
    name: "Stat",
    group: "Animated primitives",
    states: [
      { key: "row", jsx: `<Stats><Stat value="23%" label="CPU" /><Stat value={82} suffix="%" label="RAM" trend="up" /><Stat value="$1,751" label="Portfolio" trend="down" /><Stat value={405} label="Total" trend="neutral" /></Stats>` },
      { key: "long", jsx: `<Stat value="1,234,567,890" label="${LONG}" trend="up" />` },
    ],
  },
  {
    name: "MiniChart",
    group: "Animated primitives",
    states: [
      { key: "week", jsx: `<MiniChart data={[30, 45, 80, 65, 90, 40, 55]} labels={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]} />` },
      { key: "highlight", jsx: `<MiniChart data={[30, 45, 80, 65, 90, 40, 55]} labels={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]} highlight={4} />` },
      { key: "many", note: "30 bars", jsx: `<MiniChart data={[3,5,8,6,9,4,5,7,2,8,9,10,4,3,6,7,8,5,4,9,10,2,3,5,6,7,8,9,4,5]} labels={["1","2","3","4","5","6","7","8","9","10","11","12","13","14","15","16","17","18","19","20","21","22","23","24","25","26","27","28","29","30"]} />` },
      { key: "empty", jsx: `<MiniChart data={[]} />` },
      { key: "zero", note: "all zeros", jsx: `<MiniChart data={[0,0,0]} labels={["a","b","c"]} />` },
    ],
  },
  {
    name: "Meter",
    group: "Animated primitives",
    states: [
      { key: "tones", note: "blue unless the agent says warning; the old color prop folds in", jsx: `<div className="space-y-3"><Meter value={23} label="CPU" /><Meter value={82} label="Memory" detail="13.1 of 16 GB" /><Meter value={94} label="Disk" detail="94%, almost full" tone="warning" /><AnimatedProgress value={50} label="Old color prop" color="auto" /><Meter value={140} label="Over 100" /></div>` },
      { key: "bare", jsx: `<Meter value={40} showValue={false} />` },
    ],
  },
  {
    name: "AnimatedNumber",
    group: "Animated primitives",
    states: [
      { key: "basic", jsx: `<AnimatedNumber value={72} suffix="°F" className="text-4xl font-bold" />` },
      { key: "money", jsx: `<AnimatedNumber value={1751.42} prefix="$" decimals={2} duration={2} className="text-2xl font-semibold" />` },
    ],
  },
  {
    name: "AnimatedCard",
    group: "Animated primitives",
    states: [
      { key: "fade-up", jsx: `<AnimatedCard animation="fade-up"><div className="text-sm">A card with the default entry.</div></AnimatedCard>` },
      { key: "glow", note: "the glow prop", jsx: `<AnimatedCard animation="scale" glow="rgb(59,130,246)"><div className="text-sm">A card with a blue glow.</div></AnimatedCard>` },
      { key: "partial", note: "shell open, children still arriving", streaming: true, jsx: `<AnimatedCard animation="fade-up">\n  <div className="space-y-3">\n    <h3 className="font-medium">Weekly Activity</h3>\n    <AnimatedProgress value={85} />\n    <MiniChart data={[30,45,80` },
      { key: "empty", jsx: `<AnimatedCard></AnimatedCard>` },
    ],
  },
  {
    name: "AnimatedList",
    group: "Animated primitives",
    states: [
      { key: "items", jsx: `<AnimatedList gap={2}><div className="text-sm">First thing</div><div className="text-sm">Second thing</div><div className="text-sm">Third thing</div></AnimatedList>` },
    ],
  },
  {
    name: "Effects",
    group: "Effects",
    states: [
      { key: "all", jsx: `<div className="flex flex-wrap items-center gap-4"><GlowBadge color="green">Online</GlowBadge><GlowBadge color="yellow">Check</GlowBadge><GlowBadge color="red">Down</GlowBadge><ShimmerText className="text-lg font-semibold">Headline value</ShimmerText><PulseRing color="rgba(59,130,246,0.4)" size={40} /><Confetti count={12} /></div>` },
      { key: "dividers", jsx: `<div className="space-y-4"><AnimatedDivider /><AnimatedDivider variant="rainbow" /><AnimatedDivider variant="blue" /><AnimatedDivider variant="green" /></div>` },
    ],
  },
  {
    name: "Buttons",
    group: "Actions",
    states: [
      { key: "row", jsx: `<div className="flex flex-wrap gap-2"><OpenButton path="~/Downloads" label="Open Downloads" /><OpenButton url="https://junebug.ai" label="Open website" /><OpenButton app="Safari" /><QueryButton query="Delete duplicate files in Downloads" label="Clean duplicates" /><CopyButton text="npm install juno" label="Copy command" /></div>` },
      { key: "variants", jsx: `<div className="flex flex-wrap gap-2"><QueryButton query="x" label="Primary" primary /><QueryButton query="x" label="Secondary" /><CopyButton text="x" label="Copy" /></div>` },
      { key: "no-label", note: "QueryButton without a label shows the query", jsx: `<QueryButton query="Show me more details about the files you moved and why" />` },
    ],
  },
  {
    name: "Passthrough",
    group: "shadcn passthrough",
    states: [
      { key: "card", jsx: `<Card><CardHeader><CardTitle>Card title</CardTitle><CardDescription>A shadcn card the agent can use directly.</CardDescription></CardHeader><CardContent><p className="text-sm">Body text.</p></CardContent><CardFooter><Button size="sm">Action</Button></CardFooter></Card>` },
      { key: "alert-badge", jsx: `<div className="space-y-3"><Alert><AlertTitle>Heads up</AlertTitle><AlertDescription>An alert with a title and a description.</AlertDescription></Alert><div className="flex gap-2"><Badge>Default</Badge><Badge variant="outline">Outline</Badge><Badge variant="secondary">Secondary</Badge><Badge variant="destructive">Destructive</Badge></div></div>` },
      { key: "table", jsx: `<Table><TableHeader><TableRow><TableHead>Folder</TableHead><TableHead className="text-right">Files</TableHead></TableRow></TableHeader><TableBody><TableRow><TableCell>Images</TableCell><TableCell className="text-right">23</TableCell></TableRow><TableRow><TableCell>Documents</TableCell><TableCell className="text-right">45</TableCell></TableRow></TableBody></Table>` },
      { key: "accordion-avatar", jsx: `<div className="space-y-3"><div className="flex items-center gap-2"><Avatar><AvatarFallback>LM</AvatarFallback></Avatar><span className="text-sm">Lacy Morrow</span></div><Accordion type="single" collapsible><AccordionItem value="a"><AccordionTrigger>What moved?</AccordionTrigger><AccordionContent>Images, documents and videos.</AccordionContent></AccordionItem></Accordion><Progress value={40} /></div>` },
      { key: "tabs", jsx: `<Tabs defaultValue="a"><TabsList><TabsTrigger value="a">One</TabsTrigger><TabsTrigger value="b">Two</TabsTrigger></TabsList><TabsContent value="a"><p className="text-sm">Tab one.</p></TabsContent><TabsContent value="b"><p className="text-sm">Tab two.</p></TabsContent></Tabs>` },
    ],
  },
  {
    name: "Form",
    group: "Forms",
    states: [
      { key: "reminder", note: "values go back as the person's next message (dispatch_query)", jsx: `<Form title="New reminder" submit="Create reminder" intro="Create this reminder:">\n  <Field label="What"><Input name="what" placeholder="Call Maya" /></Field>\n  <Field label="When" hint="Words are fine"><Input name="when" placeholder="Tomorrow 9 AM" /></Field>\n  <Field label="List">\n    <Select name="list" defaultValue="Reminders">\n      <SelectTrigger><SelectValue /></SelectTrigger>\n      <SelectContent>\n        <SelectItem value="Reminders">Reminders</SelectItem>\n        <SelectItem value="Work">Work</SelectItem>\n      </SelectContent>\n    </Select>\n  </Field>\n  <Field label="Alert me"><Switch name="alert" defaultChecked /></Field>\n</Form>` },
      { key: "all-controls", jsx: `<Form title="Trip details" submit="Plan the trip">\n  <Field label="Where"><Input name="where" /></Field>\n  <Field label="Notes"><Textarea name="notes" placeholder="Anything I should know" /></Field>\n  <Field label="Budget"><Slider name="budget" min={0} max={5000} step={100} defaultValue={[1500]} /></Field>\n  <Field label="Book flights too"><Checkbox name="flights" /></Field>\n</Form>` },
      { key: "streaming", note: "shell open, fields still arriving", streaming: true, jsx: `<Form title="New reminder" submit="Create reminder">\n  <Field label="What"><Input name="what" /></Field>\n  <Field label="When"><Inp` },
      { key: "alone", note: "a control outside a Form draws nothing (the dead-control defect)", jsx: `Here is an input:\n\n<Input name="x" placeholder="Should not appear" />\n\n<Switch name="y" />` },
      { key: "empty", note: "no fields: nothing", jsx: `<Form title="Nothing to ask" submit="Send"></Form>` },
    ],
  },
  {
    name: "Pipeline",
    group: "Pipeline",
    states: [
      { key: "pending-unknown", note: "unknown tag still arriving", streaming: true, jsx: `<SwatchPicker colors={[{hex: "#7A9E7E", name: "Sag` },
      { key: "unknown-closed", note: "an unregistered tag that closes becomes its words as prose", jsx: `Pick one:\n\n<SwatchPicker colors={[{hex: "#7A9E7E", name: "Sage"}, {hex: "#C9A47E", name: "Clay"}]} />` },
      { key: "colour-classes", note: "hand-rolled colour, gradient and uppercase classes are dropped", jsx: `<div className="rounded-xl bg-gradient-to-br from-amber-50 to-orange-50 p-3 text-green-700"><div className="text-xs font-medium uppercase tracking-wide text-purple-500">San Francisco</div><div className="text-4xl font-bold text-red-600">54°</div><CheckCircle className="text-green-500" /> Rain</div>` },
      { key: "fallback-keeps-words", note: "malformed close: the card's name and labels survive", jsx: `<TaskSummaryCard title="Organized Downloads" tasks={[{label: "23 images moved", done: true}, {label: "45 documents sorted", done: true}` },
      { key: "skeleton-compact", note: "short cards get one line", streaming: true, jsx: `<LinkCard url="https://developer.apple.com/design/human-interface-guidelines/wid` },
      { key: "mixed", note: "prose, card, prose, streaming cursor", streaming: true, jsx: `Here is what I found.\n\n<TaskSummaryCard title="Checks" tasks={[{label: "Build", done: true}, {label: "Tests", done: false}]} />\n\nTwo of three passed and the` },
    ],
  },
];

// ── Surfaces ─────────────────────────────────────────────────────────

type Surface = "main" | "pill" | "island";

function Frame({ surface, theme, children }: { surface: Surface; theme: "light" | "dark"; children: React.ReactNode }) {
  if (surface === "island") {
    return (
      <div
        className="dark flex flex-col overflow-hidden bg-black text-white"
        data-surface="island"
        style={{
          width: 360,
          borderRadius: 20,
          boxShadow: "0 0 0 0.5px rgba(255,255,255,0.22), 0 1px 2px rgba(0,0,0,0.5), 0 4px 12px rgba(0,0,0,0.35)",
        }}
      >
        <header className="flex shrink-0 items-center gap-2.5 pl-4 pr-3" style={{ height: 36 }}>
          <span className="size-2 rounded-full bg-white/70" />
          <span className="min-w-0 flex-1 truncate text-[13px] tracking-[-0.01em] text-white/55">Organize my downloads</span>
          <span className="size-[22px] rounded-full border border-white/35" />
        </header>
        <div className="px-4" style={{ paddingBottom: 12 }}>
          <div className="text-[13px] leading-[1.55] text-white/85">{children}</div>
        </div>
      </div>
    );
  }
  if (surface === "pill") {
    return (
      <div
        className="dark flex w-[419px] flex-col overflow-hidden rounded-2xl bg-background text-foreground"
        style={{ boxShadow: "0 0 0 0.5px rgba(255,255,255,0.18), 0 8px 24px rgba(0,0,0,0.4)" }}
      >
        <header className="flex h-8 shrink-0 items-center border-b border-white/[0.06] pl-4 pr-2">
          <span className="text-[11px] tracking-[0.04em] text-white/35">JUNO</span>
        </header>
        <div className="px-6 py-4 text-sm">{children}</div>
      </div>
    );
  }
  return (
    <div className={cn(theme === "dark" && "dark", "bg-background text-foreground")} style={{ width: 720 }}>
      <div className="px-6 py-4 text-sm">
        {/* The real assistant wrapper, so card width behaves as in a conversation. */}
        <Message from="assistant">
          <MessageContent>{children}</MessageContent>
        </Message>
      </div>
    </div>
  );
}

function readQuery() {
  const q = new URLSearchParams(window.location.search);
  const surface = (q.get("surface") ?? "main") as Surface;
  const theme = (q.get("theme") ?? "light") as "light" | "dark";
  return {
    surface: ["main", "pill", "island"].includes(surface) ? surface : "main",
    theme: theme === "dark" ? "dark" : "light",
    only: q.get("c"),
    state: q.get("state"),
    media: (q.get("media") ?? "playing") as MediaMood,
  } as const;
}

// Install on import, as the bar bench does: the root VoiceProvider registers
// listeners before this route renders, and they need the internals to exist.
if (typeof window !== "undefined") {
  installGalleryTauri(readQuery().media);
}

export default function ComponentGallery() {
  const query = useMemo(readQuery, []);

  const entries = CATALOGUE.filter((e) => !query.only || e.name === query.only);
  const pageDark = query.surface !== "main" || query.theme === "dark";

  return (
    <div
      className={cn("min-h-screen px-6 py-6 font-sans", pageDark ? "bg-[#2a2a2e] text-white" : "bg-[#e9e9ec] text-black")}
      data-testid="component-gallery"
    >
      <h1 className="mb-1 text-base font-semibold">Agent components: {query.surface}, {query.theme}</h1>
      <p className="mb-6 text-xs opacity-60">
        Every tag in <code>availableComponents</code>, rendered through <code>MixedContentRenderer</code>. ?surface=main|pill|island
        &amp;theme=light|dark &amp;c=Name &amp;state=key &amp;media=playing|paused|stopped|not_running|loading|error
      </p>
      <div className="flex flex-col gap-8">
        {entries.map((entry) =>
          entry.states
            .filter((s) => !query.state || s.key === query.state)
            .map((s) => (
              <section key={`${entry.name}-${s.key}`} data-shot={`${entry.name}--${s.key}`} className="w-fit">
                <div className="mb-2 flex items-baseline gap-2 font-mono text-[11px] opacity-70">
                  <span className="font-semibold">{entry.name}</span>
                  <span>{s.key}</span>
                  {s.streaming && <span>(streaming)</span>}
                  {s.note && <span className="opacity-70">{s.note}</span>}
                </div>
                <Frame surface={query.surface} theme={query.theme}>
                  <MixedContentRenderer content={s.jsx} isStreaming={!!s.streaming} />
                </Frame>
              </section>
            )),
        )}
      </div>
    </div>
  );
}
