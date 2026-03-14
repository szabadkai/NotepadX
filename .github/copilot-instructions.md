# NotepadX — Agent Workflow

This project uses specialized agents. Consult them at the right phase — don't skip steps.

## The Team

| Agent | Role | Phase |
|-------|------|-------|
| `@pm` | Decides *what* to build and *why* | Planning |
| `@ux-designer` | Decides *how it should work* for the user | Planning |
| `@architect` | Decides *how it should be structured* in code | Planning → Review |
| `@bob` | Enforces clean code, catches structural rot | Implementation → Review |
| `@gaben` | QA — verifies visual correctness and spec compliance | Post-implementation |
| `@scribe` | Summarizes changes, updates docs, checks off plans | Close |

## Feature Workflow

When implementing a new feature or significant change, follow this sequence:

### 1. Plan (before writing code)

- **Start with `@pm`** to validate the feature is worth building, understand user impact, and define scope.
- **Then `@ux-designer`** to define the user-facing flow, interactions, and edge cases.
- **Then `@architect`** to review the plan against the codebase architecture. The architect checks: where state lives, which modules are affected, what invariants must hold, and whether the approach is the simplest correct one. **No plan ships without architect review.**

### 2. Implement

- Write the code. Use the default agent.
- For non-trivial changes spanning multiple modules, consult `@architect` on structure *before* committing to an approach.
- Mid-implementation, if you notice duplication or growing complexity, consult `@bob` for structural guidance.

### 3. Review (before merging)

- **`@bob`** reviews for clean code: SOLID violations, duplication, god functions, coupling.
- **`@architect`** reviews for correctness: invariant preservation, boundary violations, state ownership.
- **`@gaben`** reviews UI changes: visual correctness, keyboard handling, spec compliance against `docs/ux/`, accessibility.

### 4. Close (after reviews pass)

- **`@scribe`** summarizes what changed, updates affected documentation (`README.md`, `docs/ux/` specs), and checks off completed items in any written plans. **Every task ends with the scribe.**

## Quick Reference: When to Invoke Each Agent

**`@pm`** — "Should we build this?" / "How does this compare to Sublime/Zed?" / "Write the changelog entry."

**`@ux-designer`** — "How should this feature feel?" / "What's the interaction flow?" / "Accessibility review."

**`@architect`** — "Does this fit the architecture?" / "Where should this state live?" / "Review this plan." / "What breaks if we change this?"

**`@bob`** — "This function is getting long." / "I'm seeing duplication." / "Review this code for clean code issues."

**`@gaben`** — "Does this match the spec?" / "Check this UI at different resolutions." / "Final sign-off on this UI change."

**`@scribe`** — "Summarize what we just did." / "Update the docs." / "Check off the plan." / "Write a commit message."

## Rules

- **Bug fixes** don't need the full pipeline. Fix it, have `@bob` glance at it if it touches multiple modules.
- **Refactors** skip PM and UX. Start with `@architect` for structural direction, `@bob` for execution.
- **UI-only changes** skip PM if scope is cosmetic. Go: `@ux-designer` → implement → `@gaben`.
- **The architect reviews every plan.** This is non-negotiable. Plans that skip architectural review accumulate structural debt.
- **The scribe closes every task.** Summaries, doc updates, and plan checkboxes don't do themselves. Call `@scribe` when the work is done.
