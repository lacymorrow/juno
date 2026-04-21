# Juno Marketing & Growth Strategy

> Owner: Marketing Lead | Last updated: 2026-04-20
> Product: Juno — AI Desktop Automation for macOS
> Website: junebug.ai | GitHub: github.com/lacymorrow/juno

---

## Executive Summary

Juno is a native macOS app that gives AI full control of the desktop via Anthropic's Computer Use API. The product has two surfaces: a consumer desktop app (junebug.ai) and a developer CLI/MCP tool (`juno-cua`). This strategy covers both.

**Core positioning:** Juno is the first native desktop app for AI computer use. Not a Docker container. Not a browser extension. A real Mac app that works with every app on your computer.

**Primary growth levers:**
1. Demo virality (watching AI control a desktop is inherently shareable)
2. Developer distribution (`npx juno-cua` as the gateway drug)
3. SEO capture (comparison pages, use-case pages, long-tail keywords)
4. Community-driven word of mouth (HN, Reddit, Twitter/X, Discord)

---

## Target Audiences

### Audience 1: Power Users / Productivity Enthusiasts
- **Who:** Mac power users, automation enthusiasts, people who use Keyboard Maestro/Alfred/Raycast
- **Pain:** Building macros is tedious; they break when apps update; multi-app workflows are hard to script
- **Message:** "Stop building macros. Just describe what you want."
- **Channels:** r/MacApps, r/productivity, Product Hunt, Twitter/X, YouTube productivity channels
- **Conversion path:** Demo video → junebug.ai → Download DMG

### Audience 2: AI / Developer Community
- **Who:** AI engineers, developers building with LLMs, MCP ecosystem builders, Claude Code/Cursor/Codex users
- **Pain:** AI coding agents can't see or interact with the desktop; testing UI requires manual work
- **Message:** "Give your AI agent eyes and hands. `npx juno-cua`."
- **Channels:** Hacker News, r/Anthropic, r/LocalLLaMA, Dev.to, GitHub, Twitter/X AI community
- **Conversion path:** Show HN / technical post → GitHub stars → `npx juno-cua` → Desktop app

### Audience 3: Non-Technical Users (Future — post product-market fit)
- **Who:** Knowledge workers, small business owners, people who hate repetitive computer work
- **Pain:** Spending hours on form-filling, data entry, research, file management
- **Message:** "Tell your computer what to do. It does it."
- **Channels:** TikTok, YouTube Shorts, Instagram Reels, word of mouth
- **Conversion path:** Viral short-form video → junebug.ai → Download

**Current focus: Audiences 1 and 2.** Audience 3 is post-PMF when the product is polished enough for non-technical users.

---

## Positioning & Messaging

### One-liner
"AI that controls your Mac."

### Elevator pitch (30 seconds)
Juno is a native Mac app where you describe a task and AI does it — it sees your screen, moves the mouse, types, clicks, and navigates any app. Voice-activated, multi-agent, works with your entire desktop. Also works as a tool for other AI agents via MCP.

### Key differentiators (vs. competitors)
| Differentiator | Why it matters |
|----------------|---------------|
| Native macOS (Tauri + Rust, ~15MB) | Not Electron bloat, not a Docker container, not a browser extension |
| Works with ANY app | Not limited to browser tabs — Finder, Figma, Slack, Xcode, everything |
| Voice control (local Whisper) | Hands-free, private (audio never leaves device), 99+ languages |
| Multi-agent orchestration | Complex tasks split across specialist agents running in parallel |
| MCP integration (both client + server) | Extensible, and other AI agents can use Juno as a tool |
| CLI for AI agents (`juno-cua`) | Unique — gives coding agents desktop superpowers |
| Source-available (FSL-1.1-MIT) | Transparency, trust, community contributions |

### Competitive landscape
| Competitor | Type | Juno's advantage |
|------------|------|-----------------|
| Anthropic's own Computer Use demos | Docker/VNC reference impl | Native macOS, works with real desktop, ships as product |
| OpenAI Operator | Browser-only agent | Works beyond browser — any app on the desktop |
| Google Project Mariner | Browser extension | Same — desktop-wide, not browser-only |
| Keyboard Maestro | Macro builder | No scripting needed, AI adapts to UI changes |
| macOS Shortcuts | Built-in automation | Works with any app, handles complex multi-step tasks |
| Raycast AI | Launcher + AI | Juno controls the full desktop, not just launcher actions |

---

## Channel Strategy

### Tier 1: High-Impact Launch Channels (Week 1-2)

| Channel | Asset | Timing | Goal |
|---------|-------|--------|------|
| Hacker News | Show HN post | Tuesday 9-10am ET | 100+ upvotes, front page |
| Twitter/X | Launch thread (6 tweets) | Same day, 1-2hr after HN | 500+ impressions, 50+ retweets |
| Reddit | r/artificial, r/MacApps, r/Anthropic | Staggered over 3 days | Drive GitHub stars + downloads |
| Product Hunt | Full launch | 1 week after HN (momentum) | Top 5 of the day |
| Dev.to | Technical article | 1-2 days after HN | SEO + developer credibility |

### Tier 2: Sustained Growth Channels (Ongoing)

| Channel | Cadence | Content type |
|---------|---------|-------------|
| Twitter/X (@lacymorrow) | 3-5x/week | Demo clips, dev updates, engagement |
| YouTube | 2x/month | Tutorials, use-case demos, architecture deep-dives |
| GitHub | Continuous | README polish, releases, discussions, CONTRIBUTING.md |
| Blog (junebug.ai/blog) | 2x/month | Technical posts, use cases, release notes |
| SEO pages (junebug.ai) | Build out, then maintain | Comparison pages (/vs/X), use-case pages (/use-cases/Y) |
| Email newsletter | 2x/month | Release updates, tips, community highlights |

### Tier 3: Experimental / Future Channels

| Channel | When | Why |
|---------|------|-----|
| YouTube Shorts / TikTok / Reels | When demo video pipeline is solid | Viral potential for "AI controls computer" content |
| Discord community | After 500+ users | Direct feedback, support, community building |
| Podcast appearances | After launch buzz | Indie hacker pods (Indie Hackers, My First Million, etc.) |
| Conference talks | After 3-6 months | Tauri conf, AI conferences, macOS dev meetups |
| Partnerships (MCP ecosystem) | Ongoing | Co-marketing with MCP tool builders |

---

## Growth Flywheel

```
Demo video (viral) → GitHub stars → Downloads/installs
       ↓                                    ↓
  Social proof                        User creates content
       ↓                                    ↓
  More coverage ←──── Word of mouth ←── User shares demo
       ↓
  SEO ranking improves
       ↓
  Organic search traffic → Downloads
```

**The demo video is the engine.** Every marketing effort ultimately needs a compelling visual of AI controlling a desktop. This is the #1 investment.

---

## Launch Sequence (Recommended Order)

### Pre-launch (1 week before)
- [ ] Record 3-5 demo videos (30-60s each, different use cases)
- [ ] Polish README with demo GIF at top
- [ ] Set up junebug.ai/blog (even if just 1 post)
- [ ] Ensure waitlist capture works (Resend integration)
- [ ] Prepare all launch assets (HN, Twitter, Reddit, PH drafts — DONE)
- [ ] Set up basic analytics (Plausible or PostHog on junebug.ai)
- [ ] Create GitHub Discussions + enable "Discussions" tab

### Launch Day (Day 0)
1. **9:00 AM ET** — Post Show HN
2. **10:30 AM ET** — Post Twitter/X thread, tag @AnthropicAI
3. **11:00 AM ET** — Monitor HN, respond to every comment within 30 min
4. **Afternoon** — Post to r/artificial

### Post-launch (Days 1-7)
- Day 1: Post to r/MacApps
- Day 1-2: Publish Dev.to article
- Day 2: Post to r/Anthropic
- Day 3-5: Follow-up Twitter content (individual use case demos)
- Day 7: Product Hunt launch

### Sustained (Weeks 2+)
- Weekly demo videos on Twitter/X
- Biweekly YouTube tutorials
- Biweekly email newsletter
- Monthly SEO page additions
- Ongoing GitHub community engagement

---

## Content Pillars

All content falls into one of these five pillars:

### 1. Demo / Wow Factor
"Watch AI control a computer" — the inherently viral content
- Short demo clips (15-60s)
- Full task walkthroughs (2-5 min)
- Before/after comparisons (manual vs. Juno)

### 2. Technical Deep-Dives
"How it works under the hood" — developer trust + SEO
- Architecture posts (multi-agent, Tauri + Rust, MCP)
- Computer Use API insights
- Performance optimization stories

### 3. Use Cases / Tutorials
"Here's exactly how to use it for X" — drives adoption
- Form filling, research, file organization, app navigation
- Integration guides (Claude Code + Juno, Cursor + Juno)
- Voice control setup and tips

### 4. Behind the Scenes / Indie Dev
"Building in public" — community connection
- Dev logs, design decisions, challenges
- Metrics sharing (downloads, stars, revenue if applicable)
- Roadmap and feature voting

### 5. Comparison / SEO
"Juno vs X" — captures search traffic
- vs. Keyboard Maestro, Shortcuts, Raycast, Operator, etc.
- "Best AI automation tool for Mac" type pages
- Use-case landing pages

---

## Key Metrics

| Metric | Target (Month 1) | Target (Month 3) | Target (Month 6) |
|--------|-------------------|-------------------|-------------------|
| GitHub stars | 1,000 | 3,000 | 10,000 |
| DMG downloads | 500 | 2,000 | 5,000 |
| npm installs (juno-cua) | 200 | 1,000 | 5,000 |
| junebug.ai monthly visitors | 5,000 | 15,000 | 50,000 |
| Twitter followers (@lacymorrow) | +500 | +2,000 | +5,000 |
| Email list subscribers | 500 | 2,000 | 5,000 |
| Discord members | — | 200 | 1,000 |
| Product Hunt upvotes | Top 5 daily | — | — |
| HN upvotes | 100+ | — | — |

---

## Budget & Resources

### Zero-cost channels (prioritize these)
- GitHub (README, Discussions, releases)
- Twitter/X (organic posts)
- Hacker News (Show HN)
- Reddit (organic posts, not ads)
- Dev.to / Hashnode (cross-posted articles)
- Product Hunt (free launch)
- SEO pages on junebug.ai
- Email via Resend (free tier: 3,000/month)

### Low-cost investments
- Screen recording software (OBS — free, or ScreenFlow — $169)
- Analytics (Plausible — $9/mo, or PostHog free tier)
- Demo video editing (CapCut — free)
- Custom OG images / social cards (Figma — free tier)

### If budget becomes available
- Sponsored content with AI/productivity YouTubers ($500-2,000/video)
- Twitter/X promoted posts for demo videos ($100-500/campaign)
- Product Hunt "upcoming" promotion
- Conference sponsorship/speaking

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Anthropic ships competing product | High | Emphasize source-available, MCP extensibility, desktop-wide (not browser-only), community |
| macOS-only limits TAM | Medium | Windows/Linux on roadmap; `juno-cua` CLI works cross-platform for agent integration |
| API costs scare users | Medium | Clear pricing page, usage estimator, highlight that voice is local/free |
| Privacy concerns (screenshots to API) | Medium | Transparent docs, permission system, local Whisper for voice, future local model support |
| Demo video doesn't go viral | Medium | Multiple formats, A/B test hooks, consistency > single moonshot |

---

## Existing Assets (Created 2026-04-17)

| Asset | File | Status |
|-------|------|--------|
| Twitter/X launch thread | `marketing/twitter-launch-thread.md` | Ready |
| Show HN post | `marketing/show-hn.md` | Ready |
| Dev.to technical article | `marketing/devto-article.md` | Ready |
| Reddit posts (3 subreddits) | `marketing/reddit-posts.md` | Ready |
| SEO comparison pages (4 pages) | `marketing/seo-comparison-pages.md` | Ready — needs implementation on junebug.ai |

## Assets To Create

| Asset | File | Priority |
|-------|------|----------|
| Content calendar | `marketing/content-calendar.md` | P0 |
| Product Hunt launch plan | `marketing/product-hunt.md` | P0 |
| Email sequences | `marketing/email-sequences.md` | P1 |
| Video content plan | `marketing/video-plan.md` | P1 |
| Community strategy | `marketing/community-strategy.md` | P1 |
| Metrics framework | `marketing/metrics-framework.md` | P2 |
