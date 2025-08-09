# Documentation Consolidation Plan

Status: Phase 1 (non-destructive) – Archival banners + canonical entry points

Objectives

- Single source of truth with simple, skimmable docs
- Integrate all root-level Markdown into the docs system
- Preserve all useful information; clearly label anything historical or outdated
- Validate technical claims against code before publishing

Guiding Principles

- Keep it simple: short entry docs that link to detail only when needed
- No contradictory claims: tighten wording to match code reality
- Non-destructive first pass: add banners and entry points before moving/deleting
- Use existing canonical anchors when possible (LLMs.txt, docs/rules/INDEX.md)

Canonical Entry Points (effective immediately)

- README.md → high-level intro with links
- docs/SIMPLE_DOCS.md → minimal quick-start + where-things-live
- LLMs.txt → authoritative agent guidance
- docs/rules/INDEX.md → deep rules index (existing)

Phases

1) Phase 1 – Banners and Indexes (current)
   - Add "Archived – see canonical docs" banners to noisy/conflicting root Markdown
   - Create docs/SIMPLE_DOCS.md (minimal, stable)
   - Publish this consolidation plan

2) Phase 2 – Merge & Normalize
   - For each root Markdown, decide: Merge → docs/, or Archive → docs/archive/
   - Fold duplicated content into a single canonical doc
   - Normalize terminology to match actual code (tool names, actions, versions)

3) Phase 3 – Retire & Enforce
   - Remove/relocate all remaining root Markdown into docs/
   - Add CI check (optional) to prevent new root Markdown sprawl

Validation Checklist (apply to every doc during Phase 2)

- Computer Use claims match current `src-tauri/src/agent/tools/anthropic_computer_use.rs`
- Tool names/actions match code (no non-API actions presented as official)
- Security guidance aligns with `src-tauri/src/agent/tools/basic_tools.rs`
- Constants guidance aligns with generation rules and `src/lib/constants.generated.ts`
- UI integration examples use actual commands/events (see workspace rules)

Initial Root Files Tagged for Archival Banner (Phase 1)

- COMPUTER_USE_COMPLETENESS_ANALYSIS.md
- COMPUTER_USE_COMPLETENESS_SUMMARY.md
- JUNO_AI_AGENT_TOOLS_AUDIT.md
- ANTHROPIC_COMPUTER_USE_COMPLIANCE_REPORT.md
- AGENT_MODE_ARCHITECTURE_FIX.md
- AGENT_IMPROVEMENT_ANALYSIS.md

Acceptance Criteria for Phase 1

- Simple Docs available and linked
- Above root files visibly marked Archived with pointers to canonical docs
- No content lost; only additive banners + new index

Owner: Docs Maintainer
Timeline: Phase 1 – today; Phase 2 – next pass; Phase 3 – after merge review


