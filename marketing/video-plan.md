# Video Content Plan

> Video is the #1 growth lever for Juno. Watching AI control a desktop is inherently shareable.
> Reference: `marketing/growth-strategy.md` — "The demo video is the engine."

---

## Video Categories

### 1. Demo Clips (15-60 seconds)
**Purpose:** Social media fuel — Twitter/X, Product Hunt, GitHub README
**Format:** Screen recording, no voiceover, text overlays, fast-paced
**Cadence:** 1-2 per week
**Tools:** OBS (recording) + CapCut (editing) or ScreenFlow

### 2. Tutorial Videos (3-10 minutes)
**Purpose:** YouTube SEO, onboarding, developer education
**Format:** Screen recording + voiceover (Lacy narrating)
**Cadence:** 2 per month
**Tools:** OBS + mic + simple editing

### 3. Architecture / Deep-Dive Videos (10-20 minutes)
**Purpose:** Developer credibility, YouTube long-tail SEO
**Format:** Screen recording of code + diagrams + voiceover
**Cadence:** 1 per month
**Tools:** OBS + Excalidraw (diagrams) + code editor

### 4. Short-Form Vertical (15-60 seconds)
**Purpose:** TikTok, YouTube Shorts, Instagram Reels — viral reach
**Format:** Vertical screen recording, text overlays, hook in first 2 seconds
**Cadence:** Future (when demo pipeline is smooth)

---

## Priority Demo Clips (Record First)

These are the launch-critical videos. Record all 5 before launch day.

### Clip 1: The Hero Demo (60s)
**Task:** "Fill out this job application with my resume"
**Why:** Form-filling is universally relatable. Everyone hates forms.
**Shot list:**
1. Open Juno floating bar (2s)
2. Type the task (3s)
3. Watch AI navigate to the form (5s)
4. AI fills in fields one by one — show 4-5 fields being filled (20s)
5. AI submits the form (5s)
6. Text overlay: "Juno — AI that controls your Mac" (5s)
7. End card: junebug.ai (3s)

**Text overlays:**
- Start: "What if AI could fill out forms for you?"
- Middle: [no overlay, let the action speak]
- End: "junebug.ai"

### Clip 2: Voice Control (30s)
**Task:** "Hey Juno, organize my Downloads folder"
**Why:** Voice control is the wow factor differentiator
**Shot list:**
1. Show desktop with messy Downloads folder (3s)
2. Say "Hey Juno, sort my Downloads — images to Photos, PDFs to Documents" (5s)
3. Show Juno activating (2s)
4. Files moving in Finder (15s, speed up if needed)
5. Clean, organized folders (3s)
6. End card (2s)

### Clip 3: Research Task (45s)
**Task:** "Find the 5 best-reviewed coffee shops near me"
**Why:** Shows multi-step browser automation — impressive to watch
**Shot list:**
1. Type task in Juno (3s)
2. AI opens browser (3s)
3. AI searches, navigates, reads reviews (20s, sped up)
4. Results compiled and displayed (10s)
5. End card (3s)

### Clip 4: AI Agent Integration (30s)
**Task:** Claude Code using Juno to test a UI
**Why:** Unique differentiator — the "for developers" angle
**Shot list:**
1. Terminal: Claude Code running (3s)
2. Claude Code asks Juno to take a screenshot (3s)
3. Screenshot captured and returned (5s)
4. Claude Code asks Juno to click a button (3s)
5. Button clicked, page updates (5s)
6. Text overlay: "Give your AI agent eyes and hands" (5s)
7. End card: `npx juno-cua` (3s)

### Clip 5: Multi-App Workflow (45s)
**Task:** "Download the invoice from my email and save it to my Expenses folder"
**Why:** Shows cross-app capability — the "works with any app" message
**Shot list:**
1. Type task (3s)
2. AI opens Mail (3s)
3. AI finds the invoice email (5s)
4. AI downloads attachment (5s)
5. AI opens Finder, navigates to Expenses (5s)
6. File moved (3s)
7. Text overlay: "Works with every app on your Mac" (5s)
8. End card (3s)

---

## YouTube Tutorial Series

### Video 1: "Juno in 60 Seconds" (launch day)
**Length:** 60-90s
**Content:** Fast montage of 5 different tasks being completed
**Style:** No voiceover. Text overlays. Upbeat background music.
**Purpose:** Embed in README, Product Hunt, junebug.ai hero

### Video 2: "Getting Started with Juno — Complete Setup" (Week 1)
**Length:** 5-7 min
**Content:**
1. Download and install (1 min)
2. Grant permissions (Accessibility, Screen Recording) (1 min)
3. Add Anthropic API key (1 min)
4. First task walkthrough (2 min)
5. Voice control setup (1 min)
6. Tips for best results (1 min)
**Purpose:** Reduce onboarding friction, answer common questions

### Video 3: "5 Things I Automate with Juno Every Day" (Week 2)
**Length:** 8-10 min
**Content:** 5 real daily tasks, each shown start-to-finish
1. Morning email triage
2. Expense report filing
3. Research for a meeting
4. Organizing downloaded files
5. Testing a UI change (with juno-cua)
**Purpose:** Inspiration + use cases + SEO for "AI automation Mac"

### Video 4: "Claude Code + Juno: Give Your AI Agent a Desktop" (Week 3)
**Length:** 5-8 min
**Content:**
1. What is juno-cua? (1 min)
2. Install: `npx juno-cua` (1 min)
3. Configure MCP in Claude Code / Cursor (2 min)
4. Demo: AI agent takes screenshot, clicks, fills form (3 min)
5. Use cases for developers (1 min)
**Purpose:** Developer audience growth, `juno-cua` installs

### Video 5: "Juno Architecture Deep-Dive" (Month 2)
**Length:** 15-20 min
**Content:**
1. Tauri + Rust overview (3 min)
2. Multi-agent system walkthrough (5 min)
3. Computer Use loop explained (5 min)
4. MCP integration (3 min)
5. Security model (2 min)
**Purpose:** Developer credibility, conference talk dry-run

### Video 6: "Building Custom MCP Tools for Juno" (Month 2)
**Length:** 10-12 min
**Content:** Step-by-step tutorial building a custom MCP tool server
**Purpose:** Ecosystem growth, developer engagement

---

## Production Guidelines

### Recording
- **Resolution:** 2560x1440 or 1920x1080 (standard for YouTube)
- **Frame rate:** 60fps for screen recordings (smooth cursor movement)
- **Audio:** External mic for voiceover (not laptop mic)
- **Clean desktop:** Hide personal info, use clean wallpaper, close unrelated apps
- **Cursor visibility:** Ensure AI cursor movements are clearly visible (consider cursor highlight plugin)

### Editing
- **Speed:** 1.5-2x speed for repetitive actions (scrolling, loading), normal speed for key moments
- **Text overlays:** Large, readable, contrasting color. Sans-serif font. 2-3 words max.
- **Transitions:** Simple cuts. No fancy transitions. Professional, clean.
- **Music:** Optional, low volume. Use royalty-free from YouTube Audio Library or Epidemic Sound.
- **Thumbnail:** Custom thumbnail with text + screenshot. High contrast. Face if possible.

### Short-form (TikTok/Shorts/Reels) Adaptation
- Crop horizontal recording to vertical (focus on center of screen)
- Text overlays must be larger (mobile viewing)
- Hook in first 1-2 seconds (show the result, then the process)
- End with CTA text overlay: "junebug.ai" or "Link in bio"
- Use trending audio if appropriate (but screen recordings often work better silent with text)

---

## Thumbnail Templates

### Pattern 1: Before/After
Left side: messy/manual | Right side: clean/automated
Text: "AI did this in 30 seconds"

### Pattern 2: Screen + Text
Screenshot of Juno in action + large text overlay
Text: "I told AI to control my Mac"

### Pattern 3: Terminal + Magic
Terminal with `npx juno-cua` + arrow pointing to desktop action
Text: "Your AI agent can now click buttons"

---

## Distribution Checklist (Per Video)

- [ ] Upload to YouTube with SEO title, description, tags
- [ ] Create 15-30s clip for Twitter/X
- [ ] Create vertical version for Shorts/Reels (if applicable)
- [ ] Embed in relevant blog post
- [ ] Share in GitHub Discussions
- [ ] Include in next newsletter
- [ ] Add to junebug.ai (if hero/tutorial)
