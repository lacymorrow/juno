# Marketing Metrics & KPIs Framework

> Reference: `marketing/growth-strategy.md`
> Tools: Plausible/PostHog (web analytics), GitHub API (stars/installs), Resend (email), npm (downloads)

---

## North Star Metric

**Weekly Active Users (WAU)** — the number of unique users who open Juno and complete at least one task per week.

Everything else ladders up to this. Downloads without usage is vanity. Stars without downloads is incomplete.

---

## Metric Categories

### 1. Awareness (Top of Funnel)

How many people know Juno exists?

| Metric | Source | Cadence | Month 1 Target | Month 3 Target |
|--------|--------|---------|----------------|----------------|
| junebug.ai unique visitors | Plausible/PostHog | Weekly | 5,000 | 15,000 |
| GitHub repo views | GitHub Insights | Weekly | 10,000 | 30,000 |
| Twitter/X impressions | Twitter Analytics | Weekly | 50,000 | 200,000 |
| Social mentions (Juno + juno-cua) | Manual search / Mention.com | Weekly | 20 | 100 |

### 2. Acquisition (Middle of Funnel)

How many people take a first action?

| Metric | Source | Cadence | Month 1 Target | Month 3 Target |
|--------|--------|---------|----------------|----------------|
| GitHub stars | GitHub API | Weekly | 1,000 | 3,000 |
| DMG downloads | GitHub Releases API | Weekly | 500 | 2,000 |
| npm installs (juno-cua) | npm stats | Weekly | 200 | 1,000 |
| Homebrew installs | Homebrew analytics | Monthly | 50 | 200 |
| Waitlist signups | Resend audience count | Weekly | 500 | 2,000 |
| Email subscribers (total) | Resend | Weekly | 500 | 2,000 |

### 3. Activation (Bottom of Funnel)

How many people actually use the product?

| Metric | Source | Cadence | Month 1 Target | Month 3 Target |
|--------|--------|---------|----------------|----------------|
| Weekly Active Users (WAU) | App analytics (PostHog) | Weekly | 100 | 500 |
| Tasks completed per user/week | App analytics | Weekly | 3 | 5 |
| Voice tasks (% of total) | App analytics | Monthly | 10% | 20% |
| MCP connections configured | App analytics | Monthly | 5% | 15% |
| API key configured (% of downloads) | App analytics | Weekly | 60% | 70% |

### 4. Engagement & Retention

Do people come back?

| Metric | Source | Cadence | Month 1 Target | Month 3 Target |
|--------|--------|---------|----------------|----------------|
| Day 1 retention | App analytics | Weekly | 50% | 60% |
| Day 7 retention | App analytics | Weekly | 30% | 40% |
| Day 30 retention | App analytics | Monthly | — | 25% |
| GitHub issues opened (signal of engagement) | GitHub API | Weekly | 10/week | 20/week |
| GitHub Discussions activity | GitHub | Weekly | 5/week | 15/week |
| Email open rate | Resend | Per send | 40% | 40% |
| Email click rate | Resend | Per send | 10% | 10% |

### 5. Referral & Virality

Do people tell others?

| Metric | Source | Cadence | Month 1 Target | Month 3 Target |
|--------|--------|---------|----------------|----------------|
| GitHub stars growth rate | GitHub API | Weekly | +100/week | +200/week |
| Twitter mentions (organic, not from Lacy) | Twitter search | Weekly | 5/week | 20/week |
| "How did you find Juno?" survey responses | Onboarding survey | Monthly | — | Implement |
| Referral traffic (% of junebug.ai) | Plausible | Monthly | 10% | 20% |

---

## Dashboard Layout

Set up a simple dashboard (Notion, Google Sheets, or PostHog dashboard) with these sections:

```
┌─────────────────────────────────────────────────────┐
│                  JUNO GROWTH DASHBOARD               │
├─────────────────┬───────────────────────────────────┤
│  North Star     │  WAU: ___  (trend: ↑/↓/→)        │
├─────────────────┼───────────────────────────────────┤
│  Awareness      │  Site: ___  │  GitHub views: ___  │
│                 │  Impressions: ___                  │
├─────────────────┼───────────────────────────────────┤
│  Acquisition    │  Stars: ___  │  Downloads: ___    │
│                 │  npm: ___  │  Waitlist: ___       │
├─────────────────┼───────────────────────────────────┤
│  Activation     │  API key %: ___  │  Tasks/user: _ │
│                 │  Voice %: ___                      │
├─────────────────┼───────────────────────────────────┤
│  Retention      │  D1: ___  │  D7: ___  │  D30: __ │
├─────────────────┼───────────────────────────────────┤
│  Referral       │  Organic mentions: ___             │
│                 │  Star growth rate: ___/week        │
└─────────────────┴───────────────────────────────────┘
```

---

## Channel-Specific Metrics

### Hacker News
| Metric | Target |
|--------|--------|
| Show HN upvotes | 100+ |
| Front page duration | 4+ hours |
| Comments | 30+ |
| Traffic spike to junebug.ai | 2,000+ in 24h |
| GitHub stars from HN | +200 in 48h |

### Product Hunt
| Metric | Target |
|--------|--------|
| Upvotes | 400+ |
| Rank | Top 5 daily |
| Comments | 50+ |
| Traffic spike | 5,000+ in 24h |
| Downloads from PH | 150+ |

### Twitter/X
| Metric | Target (per post avg) |
|--------|----------------------|
| Impressions | 1,000+ |
| Engagements | 50+ |
| Retweets (demo videos) | 20+ |
| Link clicks | 30+ |
| Follower growth | +50/week |

### Dev.to / Blog
| Metric | Target |
|--------|--------|
| Views (first 7 days) | 5,000+ |
| Reactions | 100+ |
| Comments | 10+ |
| Referral traffic to junebug.ai | 200+ |

### Email
| Metric | Target |
|--------|--------|
| Open rate | 40%+ |
| Click rate | 10%+ |
| Unsubscribe rate | <2% |
| Reply rate (feedback emails) | 5%+ |

---

## Measurement Cadence

| Frequency | What to review |
|-----------|---------------|
| Daily (launch week only) | HN rank, PH rank, Twitter engagement, download count |
| Weekly | WAU, stars, downloads, npm installs, site traffic, email metrics |
| Monthly | Retention curves, channel ROI, content performance ranking, goal progress |
| Quarterly | Strategy review, target adjustment, channel pruning |

---

## Implementation Priority

### Immediate (Before Launch)
1. **Plausible or PostHog on junebug.ai** — basic web analytics (visitor count, referrers, pages)
2. **GitHub Insights** — already available, just check weekly
3. **npm stats** — already available at npmjs.com/package/juno-cua
4. **Resend dashboard** — already available for email metrics

### Soon (Month 1)
5. **PostHog in Juno app** — track app opens, tasks completed, features used (privacy-conscious, self-hosted if preferred)
6. **Twitter Analytics** — available in Twitter dashboard
7. **Google Sheets dashboard** — manual weekly data entry to track trends

### Later (Month 3+)
8. **Automated data collection** — script to pull GitHub API + npm stats + Plausible API into a dashboard
9. **Onboarding survey** — "How did you find Juno?" in the app
10. **Cohort analysis** — retention by acquisition channel

---

## What NOT to Track

- Vanity metrics in isolation (stars without downloads, followers without engagement)
- Per-minute social media stats (weekly cadence is sufficient)
- Competitor metrics (focus on own growth, not theirs)
- Over-granular funnel steps (keep it simple until there's data to justify complexity)
