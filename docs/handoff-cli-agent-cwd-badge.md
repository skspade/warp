# Handoff — CLI-Agent CWD Badge Fix

**Repo:** `warpdotdev/warp` (fork: `skspade/warp`)
**Branch:** `skspade/cli-agent-cwd-badge`
**Upstream issue:** [warpdotdev/warp#9125 — Vertical tabs don't update git branch when CWD changes via external tool](https://github.com/warpdotdev/warp/issues/9125)
**Claim comment:** https://github.com/warpdotdev/warp/issues/9125#issuecomment-4444764426

> Not committed to upstream. This doc lives in the working tree on the feature branch for context only — don't include it in the PR. Stage individual code/test changes when committing; leave this file unstaged or add to `.git/info/exclude`.

---

## TL;DR

Warp's vertical-tab branch badge stays stuck on `main` when a long-running CLI agent (Claude Code, Codex, Gemini CLI, etc.) `cd`s into a git worktree on a different branch. The badge is driven by data captured during Warp's prompt-rendering cycle (`warp_precmd`), so a cwd change *inside* a running child process never propagates.

Fix: teach `TerminalView::current_git_branch` (and its `current_repo_path` sibling) to prefer the cwd already reported by the active `CLIAgentSession` over the shell's cwd. Resolve branch from that path via the existing `git_status_metadata` fs-watcher infra.

Scope: ~50 lines of Rust + an integration test under `crates/integration/`. No new escape-sequence parsing, no protocol changes, no new plugin events — the data already arrives in `CLIAgentSession::session_context.cwd`.

---

## How we got here

1. **Symptom.** Sean uses Claude Code's native worktree feature (the `EnterWorktree`/`ExitWorktree` tools). Claude `cd`s into `.../worktrees/<name>` on a feature branch, but Warp's terminal badge keeps showing `main` — the branch of the *shell's* original cwd, not Claude's.

2. **First investigation — the plugin.** Sean uses the [`claude-code-warp`](https://github.com/warpdotdev/claude-code-warp) plugin (he runs a personal fork of it). Reviewed the plugin source at `/Users/seanspade/source/claude-code-warp`:
   - It emits structured payloads on the `warp://cli-agent` OSC 777 channel.
   - `build-payload.sh` already includes `cwd` and `project` on every event.
   - `on-cwd-changed.sh` fires a `cwd_changed` event whenever Claude's cwd moves.
   - **Conclusion:** the plugin is already telling Warp the right cwd. The fix isn't on the plugin side.

3. **Considered patching Warp.** Initially thought Warp was closed-source, so this was a dead end. Sean noted Warp open-sourced their client in May 2026 (sponsored by OpenAI, AGPLv3 with MIT for `warpui_core`/`warpui` crates). Source at `github.com/warpdotdev/warp` — Rust, 58k+ stars.

4. **Located the badge codepath.** In the open-source tree:
   - `app/src/terminal/view/tab_metadata.rs:36` — `current_git_branch()` is the single source of truth for the badge.
   - It reads `ContextChipKind::ShellGitBranch` (from the shell prompt) first, then falls back to `git_status_metadata` (fs-watch on the shell's cwd).
   - Neither input sees a cwd change happening inside a long-running child process.
   - Callers: `app/src/workspace/view/vertical_tabs.rs` (~5 sites — tab strip, summary, detail sidecar), `app/src/tab.rs` (tab title strip). All read through `current_git_branch`, so the badge is stale on every surface together.

5. **Confirmed the data is already flowing.** Found `CLIAgentSession::session_context.cwd` in `app/src/terminal/cli_agent_sessions/mod.rs:40,165`. It absorbs the `cwd` field from every OSC 777 `warp://cli-agent` payload (set on `session_start`, refreshed on `cwd_changed`). So Warp *receives* the agent's cwd — it just doesn't feed it into the branch resolver.

6. **Searched existing issues.** Found **#9125** — filed by Wayne Hoover, 0 comments, `ready-to-implement`. The issue body:
   - Diagnoses the exact root cause (cwd only read during prompt cycle).
   - Explicitly references `claude-code-warp` and proposes using the OSC 777 `cwd` field.
   - Lists OSC 7 (`\e]7;file://...`) as the more general alternative.
   - Maps to a closely related bug **#9958** ("Third-party CLI agent toolbar shows stale and misleading state") whose Bug 1 is almost certainly the same root cause.

7. **Read Warp's CONTRIBUTING.md.** Relevant bits for this fix:
   - **Bugs skip the spec process.** All triaged bugs are implicitly `ready-to-implement` — #9125 already has the label.
   - **AI agents explicitly welcome.** No disclosure required, no human-only rule. Oz (Warp's agent) does the first PR review automatically, then routes to a Warp SME after Oz approves. `/oz-review` re-requests review (3× max).
   - **Tests required.** Bug fixes need a regression test. User-facing flows go in `crates/integration/`. The bar is explicitly *higher* for agent-driven contributions.
   - **Style.** `./script/presubmit` must pass (fmt + clippy + tests). Branch prefix with handle (`skspade/...`). Commit messages explain *what* and *why*.

8. **Decisions made.**
   - **Option B (cli-agent payload) over OSC 7.** Tighter scope, lands the reported case, reuses existing infra. OSC 7 is a worthy follow-up but doesn't need to be coupled to this fix.
   - **Posted claim comment.** Short version — no big design questions surfaced. "Picking this up. Will open a PR going with option B from the issue body..."
   - **Forked the repo, cloned, pruned.** `skspade/warp`, branch `skspade/cli-agent-cwd-badge` off `upstream/master`, all non-master branches deleted from the fork.

---

## Repo state at handoff

| Item | Value |
|------|-------|
| Local clone | `/Users/seanspade/source/warp` |
| Working branch | `skspade/cli-agent-cwd-badge` |
| Branched from | `upstream/master` (commit `c9237a57`) |
| `origin` | `https://github.com/skspade/warp.git` (clean — only `master` remains) |
| `upstream` | `https://github.com/warpdotdev/warp.git` |
| `remote.pushDefault` | `origin` (explicitly set; protects against accidental push to upstream) |
| Working tree | Clean. No code changes yet. |

To verify:
```bash
cd /Users/seanspade/source/warp
git remote -v
git branch -vv
git status
```

---

## The change — code map

### Primary target

**`app/src/terminal/view/tab_metadata.rs`** — `current_git_branch` (around line 36) and `current_repo_path` are the points to patch.

Current logic for `current_git_branch`:
1. Prefer `prompt_chip_value(ContextChipKind::ShellGitBranch)` (set from shell prompt).
2. Fall back (under `cfg(feature = "local_fs")`) to `git_status_metadata().current_branch_name`.

Proposed logic:
1. If an active `CLIAgentSession` exists for this terminal view *and* its `session_context.cwd` is non-empty *and* differs from the shell's cwd → resolve branch via `git_status_metadata` (or whatever fs-watch path it uses) on the agent cwd.
2. Otherwise → existing behavior.

Same shape for `current_repo_path` so the repo identity stays consistent with the displayed branch.

### Where the agent cwd lives

**`app/src/terminal/cli_agent_sessions/mod.rs`**
- Line 39 — `pub struct CLIAgentSessionContext { pub cwd: Option<String>, ... }`
- Line 165 — context cwd is updated from each incoming event's `cwd` field. So every plugin notification keeps it current.
- Line 310 — `CLIAgentSessionsModel::session(terminal_view_id)` is the lookup you'll want: given a `TerminalView`'s id, return its `CLIAgentSession` (if any).

The event schema is in `app/src/terminal/cli_agent_sessions/event/v1.rs` — `cwd` is already a first-class field (line 47, 67).

### Callers that consume the result

All read through `current_git_branch` — no caller-side changes needed, but worth grepping after the patch to confirm no inlined duplicates:

- `app/src/workspace/view/vertical_tabs.rs:2775, 3027, 3279, 5601, 6063` — vertical tab branch label (the user's primary surface), summary entries, detail sidecar, agent footer.
- `app/src/tab.rs` — tab strip title.

### Tests

- **Unit:** add coverage in `tab_metadata` for the new precedence (agent cwd present, agent cwd absent, agent cwd matches shell cwd).
- **Integration:** `crates/integration/` — drive a fake CLI-agent session through the OSC 777 channel, assert the badge reflects the agent's reported branch. Look at existing tests under `crates/integration/src/test/workspace.rs` for the patterns they use (it already calls `current_git_branch`).

---

## Build & validation

From `WARP.md` / `CONTRIBUTING.md`:

```bash
cd /Users/seanspade/source/warp

# Initial setup (one-time per machine)
./script/bootstrap

# Build & run for manual testing
cargo run

# Unit tests
cargo nextest run

# Full presubmit (must pass before pushing)
./script/presubmit
```

**Manual test plan** (include in PR description with screenshots/screencast):
1. Open Warp, open a terminal in a repo on `master`.
2. Launch Claude Code (or any CLI agent that uses the cli-agent OSC 777 channel).
3. Have the agent enter a worktree or `cd` to a path on a different branch.
4. Confirm the vertical-tab badge updates to the new branch *without* a manual prompt redraw.
5. Confirm the badge falls back to shell cwd behavior when no agent is active.
6. Confirm multi-agent same-dir scenario from #9958 Bug 1 (each agent's badge reflects its own cwd, not a shared cached value).

---

## Open questions to address in the PR description

These were drafted as questions to ask before coding, but Sean said "let the code speak." Address them in the PR body once the implementation makes the answer obvious:

1. **Precedence.** Why agent cwd wins over shell prompt chip while an agent session is active. (Expected stance: agent cwd is more current; shell prompt only updates at prompt boundaries which the agent never crosses.)
2. **UI distinction.** Whether the badge should visually distinguish "branch from agent" vs "branch from shell." (Expected stance: no — identical surface, consistent with how `git_status_metadata` is already preferred over the shell chip when both are present.)
3. **#9958 relationship.** Whether Bug 1 of #9958 is a dupe of #9125 (likely yes — same root cause), and whether this PR also closes it.

---

## Contributing workflow reminders

- **No spec PR needed** — this is a triaged bug, implicitly `ready-to-implement`.
- **Push to `origin`, not `upstream`** (pushDefault is already set, but double-check).
- **Open PR against `warpdotdev/warp:master`.** Use the [PR template](https://github.com/warpdotdev/warp/blob/master/.github/pull_request_template.md).
- **Add a changelog entry:** `CHANGELOG-BUG-FIX` line in the PR description.
- **Reviewers:** Don't assign anyone. Oz auto-assigns. After addressing Oz feedback, comment `/oz-review` (max 3×).
- **Slack:** Optional but recommended for design discussion: [`#oss-contributors`](https://warpcommunity.slack.com/archives/C0B0LM8N4DB) in the Warp community Slack. Probably not needed for a focused bug fix.
- **Escalation:** Tag `@oss-maintainers` on the issue or PR if anything stalls.

---

## Personal context worth knowing

- **Sean does not own `claude-code-warp`.** He maintains a personal fork for local patches, but the upstream plugin is owned by `warpdotdev`. **Do not reference any maintainer relationship in PR/issue text.** (Earlier draft of the claim comment incorrectly claimed ownership; corrected before posting.)
- **The reported symptom is real-world recurring** — Sean hits this every time Claude enters a worktree.
- **Style preferences (from his global CLAUDE.md):**
  - Keep implementations simple, no preoptimizations unless told.
  - Don't underscore-prefix unused variables — remove them.
  - Don't push with `--no-verify`.
  - On debugging: two failed fixes with identical metrics → stop guessing, instrument. Inspection over iteration.
  - When corrected, restate the corrected assumption in one sentence and stop explaining.

---

## Suggested first actions in the new session

1. `cd /Users/seanspade/source/warp && git status && git branch -vv` — confirm the handoff state.
2. Open `app/src/terminal/view/tab_metadata.rs` and read `current_git_branch` + `current_repo_path` in full (this file is small).
3. Open `app/src/terminal/cli_agent_sessions/mod.rs` and skim the `CLIAgentSessionsModel` + `CLIAgentSession` types — confirm `session(terminal_view_id)` returns what you need.
4. Run `./script/bootstrap` if not done yet (one-time setup).
5. Run `cargo nextest run -p warp` or similar to confirm the toolchain works before touching anything.
6. Draft the patch. Keep it focused — no surrounding cleanup, no unrelated refactors.
7. Add the integration test before the implementation if practical (TDD), since the failing test is also the regression proof Oz/SME will look for.
