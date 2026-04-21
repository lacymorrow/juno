# Community Building & Developer Advocacy Strategy

> Reference: `marketing/growth-strategy.md`

---

## Philosophy

Juno's community strategy is developer-first, founder-led, and authenticity-driven. The goal is not "community management" — it's building genuine relationships with people who care about AI desktop automation, Tauri/Rust development, and MCP tooling.

**Principle:** Show up where the audience already is. Don't build a walled garden until there's enough gravity to sustain it.

---

## Phase 1: Launch & Early Traction (Months 1-2)

### GitHub as Primary Community Hub

GitHub is where developers already are. Use it as the first community layer.

**Actions:**
- [ ] Enable GitHub Discussions on the repo
- [ ] Create discussion categories: General, Show & Tell, Ideas & Feature Requests, Q&A, Bug Reports
- [ ] Pin a "Welcome to Juno" discussion with getting started links
- [ ] Respond to every issue and discussion within 24 hours
- [ ] Add CONTRIBUTING.md with clear contributor guidelines
- [ ] Label good first issues for new contributors
- [ ] Recognize contributors in release notes

**Why GitHub first:** Developers trust it. It's indexed by search engines. It builds social proof (stars, activity, contributor count). And it doesn't require maintaining another platform.

### Twitter/X as Broadcast + Engagement Channel

Twitter is for amplification, not community. Use it to broadcast content and engage with individuals.

**Actions:**
- [ ] Follow and engage with: AI researchers, Tauri developers, MCP builders, Computer Use enthusiasts, indie hackers
- [ ] Reply to anyone who mentions Juno, Computer Use, or desktop AI automation
- [ ] Quote-tweet interesting use cases or bug reports (with their permission)
- [ ] Post 3-5x/week (see content calendar)
- [ ] Run Twitter polls on feature priorities ("What should Juno automate next?")

### Reddit as Distribution Channel

Reddit is for launch bursts and answering questions. Don't treat it as an ongoing community.

**Ongoing actions:**
- [ ] Monitor r/artificial, r/MacApps, r/Anthropic, r/LocalLLaMA for relevant discussions
- [ ] Answer questions where Juno is genuinely relevant (never spam)
- [ ] Post major releases to relevant subreddits
- [ ] Build karma in these communities through genuine participation (not just Juno posts)

### Hacker News as Credibility Channel

HN is for technical credibility and reaching early adopters.

**Ongoing actions:**
- [ ] Show HN for major releases (not every patch)
- [ ] Comment on relevant threads (Computer Use, Tauri, desktop automation, AI agents)
- [ ] Share technical insights from building Juno (without being promotional)

---

## Phase 2: Growing Community (Months 3-6)

### Discord Server (Launch at 500+ users)

**Why wait:** A Discord with 10 people feels dead. Wait until there's enough organic demand (people asking "is there a Discord?").

**Structure:**
```
#announcements — Release notes, major updates (read-only)
#general — Open discussion
#showcase — Users sharing what they built/automated
#help — Support questions
#feature-requests — Upvoteable feature ideas
#dev — Technical discussion for contributors
#mcp-tools — MCP ecosystem discussion
#off-topic — Whatever
```

**Roles:**
- @Team — Lacy + any team members
- @Contributor — Anyone who has merged a PR
- @Beta — Early testers for unreleased features

**Rules:**
1. Be respectful
2. No spam
3. Search before asking
4. Share your use cases — we love seeing what people build

**Management:**
- Lacy checks in 1-2x/day
- Auto-moderation via basic Discord bot (spam filter, welcome message)
- No full-time community manager needed at this stage

### Developer Advocacy Through Content

**Developer relations is content, not conferences.** At this stage.

**Actions:**
- Write integration guides: "How to use Juno with Claude Code," "How to use Juno with Cursor"
- Create MCP tool examples in the repo (`examples/` directory)
- Stream coding sessions building Juno features (Twitch/YouTube Live — experimental)
- Write guest posts for relevant blogs (Anthropic's blog, Tauri's blog, Vercel's blog)

### Open Source Community Building

**Actions:**
- [ ] Maintain a public roadmap (GitHub Projects or a pinned Discussion)
- [ ] Run "Feature of the Month" where community votes on next priority
- [ ] Recognize top contributors quarterly (Twitter shoutout + README mention)
- [ ] Create issue templates that make contributing easy
- [ ] Document architecture clearly enough that outsiders can contribute
- [ ] Consider a "Juno Plugins" program where community builds MCP tools

---

## Phase 3: Ecosystem & Scale (Months 6+)

### MCP Ecosystem Partnership

Juno is both an MCP client and server. The MCP ecosystem is growing. Position Juno as a key node.

**Actions:**
- Partner with MCP tool builders for co-marketing
- Create a "Works with Juno" badge for MCP tools
- Maintain a directory of Juno-compatible MCP tools on junebug.ai
- Contribute to MCP spec discussions where desktop automation is relevant

### Conference & Event Presence

**When:** After 6 months, if the product has traction.

**Target conferences:**
- Tauri Conf (if it exists)
- AI Engineer Summit
- Local macOS developer meetups
- Rust conferences (RustConf, Rust Nation)
- Indie hacker events (MicroConf)

**Talk topics:**
- "Building a Desktop AI Agent with Tauri and Rust"
- "The Multi-Agent Architecture Behind Juno"
- "Giving AI Agents Eyes and Hands: MCP + Computer Use"

### User-Generated Content Program

**Goal:** Turn power users into advocates.

**Actions:**
- Create a "Juno Showcase" page on junebug.ai featuring user stories
- Repost user demos on Twitter/X (with credit)
- Send swag to top community members (stickers, t-shirts — when budget allows)
- Feature user workflows in the monthly newsletter

---

## Developer Advocacy Metrics

| Metric | Month 1 | Month 3 | Month 6 |
|--------|---------|---------|---------|
| GitHub stars | 1,000 | 3,000 | 10,000 |
| GitHub contributors | 5 | 15 | 30 |
| Open issues resolved (% within 1 week) | 80% | 80% | 80% |
| GitHub Discussions threads | 20 | 100 | 300 |
| Twitter mentions/week | 10 | 50 | 200 |
| Discord members | — | 200 | 1,000 |
| MCP tool integrations | 3 | 10 | 25 |
| Community-authored content (posts, videos) | 2 | 10 | 30 |

---

## Key Relationships to Build

| Person/Org | Why | How |
|------------|-----|-----|
| Anthropic DevRel (Alex Albert et al.) | Computer Use API creators | Tag in launch posts, share interesting use cases, attend their events |
| Tauri team | Framework creators | Contribute upstream, share Juno as a Tauri showcase |
| MCP ecosystem builders | Co-marketing, integration partners | Build integrations, co-promote |
| AI productivity YouTubers | Distribution | Send early access, offer to collaborate on demo videos |
| Indie hacker community | Word of mouth | Build in public, share metrics, be genuine |
| macOS dev community | Technical credibility | Share Rust/macOS insights, contribute to ecosystem |

---

## Anti-patterns to Avoid

1. **Don't build a Discord too early** — empty communities feel dead
2. **Don't automate engagement** — no bots posting, no scheduled DMs, no auto-follow
3. **Don't be promotional in other communities** — earn the right to mention Juno by being genuinely helpful
4. **Don't ignore negative feedback** — address it publicly and honestly
5. **Don't over-moderate** — let conversations happen naturally
6. **Don't chase vanity metrics** — 100 engaged users > 10,000 passive followers
