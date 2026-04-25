# Lessons Learned

Ported from rssekai on 2026-04-16. These are process rules that apply to all sessions.

## 2026-04-04 — Skip execution handoff when user won't execute

**Mistake:** After writing a plan the user explicitly said they would not execute, presented
execution options anyway — copying the writing-plans skill's "Execution Handoff" section verbatim.

**Pattern:** Blindly following a skill template without adapting to user-stated intent.

**Rule:** When the user says they won't execute now or plan to execute with another tool, skip
the execution handoff entirely. Just confirm the plan is saved and committed. The skill template
is a default, not a mandate — always defer to the user's explicit instructions.

**Applied:** Any skill with a handoff/next-steps section (writing-plans, brainstorming, etc.).
Read the user's request fully before appending boilerplate.

## 2026-04-04 — Commit implementation plans immediately after writing

**Pattern:** After writing an implementation plan to `docs/plans/`, the plan file sits uncommitted
until the end of the session (or gets forgotten entirely).

**Rule:** Commit the plan file to the repository immediately after writing it — before any
implementation work begins. Do not batch the plan commit with later code changes.

**Why:** Plans are the record of intent. Committing them immediately makes them portable across
machines and available as context for other sessions.

**Applied:** Any use of /write-plan or manual plan creation. Stage and commit the `docs/plans/`
file as the last step of plan writing, before execution begins.

## 2026-04-04 — Always commit changes at end of session

**Pattern:** Code changes, plan documents, lessons updates, and any other modifications made
during a session should be committed to the repository before the session ends.

**Rule:** When any code changes are made, plans are written or executed, or lessons are updated,
all changes must be committed to the repo at the end of each session. Do not leave uncommitted work.

**Verification step:** Before completing a session, run `git status` to confirm no unstaged or
uncommitted changes remain for work done in that session.

## 2026-04-04 — Honor pause instructions across context compaction

**Mistake:** User said "Pause" and assistant acknowledged it. After context was compacted,
the continuation prompt said to "resume directly" and work began again — ignoring the user's
pause instruction that was only preserved in the summary text, not as an active directive.

**Pattern:** Context compaction loses the imperative force of user instructions. The summary
records *that* the user paused, but the continuation framing ("pick up where you left off")
overrides it.

**Rule:** When a conversation summary mentions the user requested a pause or stop, treat that
as still active. Do not resume work until the user explicitly says to continue.

**Applied:** Any session continuation after compaction. Always check the summary for
pause/stop/wait directives before taking action.

## 2026-04-04 — Never use `git commit --amend` after pushing

**Mistake:** After pushing a commit, used `git commit --amend` to fix an error instead of
making a new fix commit. This rewrote the already-pushed commit hash, causing local/remote
divergence that required a rebase with merge conflicts to resolve.

**Pattern:** `--amend` rewrites history. Once a commit is pushed, its hash is shared state.
Amending it creates a new hash locally while the old one exists on the remote, guaranteeing
divergence.

**Rule:** Never use `git commit --amend` on commits that have already been pushed. Always create
a new commit for fixes. A clean `git push` is worth more than a pretty `git log`.

**Applied:** All post-push fixes. Use a new commit (`fix: ...`) instead of amending.

## 2026-04-04 — Check existing naming conventions before creating new files

**Pattern:** When creating new plan documents or files in an existing directory, the project
may use a specific naming convention (e.g., date-prefixed kebab-case) that isn't obvious.

**Rule:** Before creating any new document in an existing directory, inspect sibling files and
match the established naming pattern (date prefix, kebab-case, descriptive slug, etc.).

**Verification step:** After creating a file, confirm its name matches the pattern of siblings
in the same directory.

## 2026-04-05 — Every memory write requires a paired tasks/lessons.md commit

**Pattern:** Feedback saved only to `~/.claude/projects/.../memory/` is machine-local and not
portable. This rule has been written before and keeps being skipped.

**Rule:** Every feedback memory write is a two-step operation — both steps are mandatory:
1. Write the memory file under `memory/`
2. Append the same rule to `tasks/lessons.md` and commit immediately

**Why:** Memory is machine-local. `tasks/lessons.md` is git-tracked and portable. Skipping
step 2 means the lesson is lost on any other machine or session.

**How to apply:** After writing any memory file, immediately open `tasks/lessons.md`, append
the entry, and commit. Treat step 2 as atomic with step 1 — there is no valid reason to defer.

**Verification step:** Before considering any memory-write complete, confirm `tasks/lessons.md`
has been updated and `git status` is clean.

## 2026-04-06 — No wide-breadth codebase exploration

**Mistake:** Before writing a plan, launched a broad codebase exploration agent that read every
handler, service, and component file. This consumed the entire session's usage budget before
a single line of the plan was written.

**Pattern:** Treating exploration as "free" — exploring everything upfront rather than reading
only what the task directly touches.

**Rule:** Never explore speculatively. Before touching any file:
1. Check `tasks/codebase-filemap.md` — the description may be enough to write the code
2. Read only the 2–4 files the current task directly modifies
3. If uncertain which files are relevant, ask the user rather than exploring

**Why:** Wide exploration burns the entire context window. Plans and implementations should be
written incrementally — one task group at a time — reading only what that task requires.

**Applied:** All sessions. Especially plan-writing, where it is tempting to "understand
everything" before starting. The file map exists precisely to avoid this.

## 2026-04-06 — Keep a codebase file map in the repo

**Rule:** A lightweight index of every significant file lives at `tasks/codebase-filemap.md`.
It records what each file owns, known gaps, and build commands. Update it when files are
created, deleted, or significantly changed.

**Why:** Avoids re-exploring the codebase every session. The map is the first thing to check
before reading any file. It is git-tracked and portable across machines.

**Applied:** All sessions. Check the map before opening any file.

## 2026-04-10 — Update the codebase filemap inline, never as deferred cleanup

**Pattern:** Files were created or changed without updating `tasks/codebase-filemap.md` at the
time. The gap was only caught at a later review.

**Rule:** Update `tasks/codebase-filemap.md` at the moment a file is created, deleted, or gets a
significant change (new routes, new model fields, new public API, ownership shift). Commit the
filemap change in the same commit as the code, or immediately after. Never defer to end of session.

**Why:** Deferred updates get missed. The filemap is only useful if it reflects the current state
of the codebase. Inline updates eliminate drift entirely.

**Verification step:** Before committing any code change, confirm that `tasks/codebase-filemap.md`
already reflects the file being added, removed, or changed.

**Applied:** All sessions. Creating a file → add entry. Deleting → remove entry. Changing public
API → update the description on the spot.

## 2026-04-10 — Commit at batch boundaries; group logically within a batch

**Rule:** A clean working tree is required when moving from one implementation batch to the next.
Within a batch, group related changes into a small number of logical commits — not one per file
and not one per step.

**Why:** One commit per file or per step creates excessive history noise. One commit per batch
is the right unit of progress. Dangling changes at batch boundaries get missed.

**How to apply:**
- One batch → one to a few logical commits, grouped by concern — not by language or layer
- A review-fix cycle for a batch = one commit, even when fixes span multiple files
- Never one commit per file or per numbered step inside a batch
- `git status` must be clean **before starting** a batch and **when the batch ends**

## 2026-04-10 — Steps within a batch are not individual tasks requiring confirmation

**Mistake:** Implementer subagents pause between steps within a batch asking "shall I continue?"
— treating each numbered step as a stopping point.

**Rule:** Steps within an implementation batch are a recipe, not a task list. The subagent must
execute all steps in the batch autonomously from start to finish without pausing. The only valid
pause points are: (a) before starting work if there are genuine blocking questions, and (b) at
batch boundaries when the batch is fully complete.

**Why:** Mid-batch pauses break flow, require manual shepherding for routine work, and defeat the
purpose of autonomous execution.

## 2026-04-11 — Always check existing migration numbers before creating a new migration

**Pattern:** A migration file was created without checking the existing highest number, causing
a version collision that broke database startup.

**Rule:** Before creating any migration file, list `migrations/` to find the highest existing
number. The new file must be `{max+1}_name.sql`. Never assume the next number — always check
the directory.

**Why:** The migration runner determines version from the numeric prefix. Two files with the same
prefix are a collision.

**Verification step:** After creating a migration file, confirm no other file in the directory
shares the same numeric prefix.

## 2026-04-11 — Present changes for review before implementing

**Mistake:** User described several UI issues. Before they could review what the proposed fixes
were, an agent was dispatched to implement them all immediately.

**Rule:** When a user describes issues or asks for fixes, present the diagnosis and proposed
approach first. Wait for explicit approval before dispatching implementation agents or writing
any code.

**Why:** The user can catch misunderstandings early. Implementing before review wastes effort
and creates correction commits.

**How to apply:** After diagnosing issues, output a structured summary of root causes and
proposed fixes. Only proceed to implementation after the user confirms.

## 2026-04-13 — All new development must happen on scoped branches, not main

**Rule:** Every implementation task must start by creating and switching to a scoped git branch
named after the version or feature (e.g., `0.1.0`, `feature/some-name`). No implementation
changes are committed directly to `main`.

**Why:** Clean separation of in-progress work from the stable main branch; makes PRs, reviews,
and rollbacks tractable.

**How to apply:** Before writing a single line of implementation code, `git checkout -b <branch>`.
Only documentation, plans, and config that belong on `main` are committed there directly.

## 2026-04-15 — Present a plan and wait for approval before implementing

**Mistake:** User described several problems. Before presenting any plan or diagnosis, code
changes were made across multiple layers without the user seeing what was proposed.

**Rule:** For any non-trivial task (more than ~1–2 file edits, involves design decisions, or
touches multiple layers), write and present a plan first, then stop and wait for explicit
approval before touching any file.

**Why:** The user has called this out more than once. Launching straight into implementation
bypasses their review of approach, scope, and trade-offs.

**How to apply:** When a request involves multiple files or architectural choices, output the
plan (what will change and why, file by file), end the response, and wait. The project has
`/write-plan` and `/execute-plan` skills for exactly this workflow.

## 2026-04-16 — When user calls out a missing memory action, read tasks/lessons.md first

**Mistake:** User said "you failed to commit, as noted in memory." Instead of reading
`tasks/lessons.md`, only the memory file was read — missing the broader rule documented there.

**Rule:** When the user calls out a missing or incomplete memory/commit action, open
`tasks/lessons.md` before responding. The full rule lives there, not just in the scoped memory
file.

**Why:** Memory files can be narrowly scoped. `tasks/lessons.md` is the authoritative git-tracked
record and often states the rule more completely. Skipping it means acting on a partial
understanding of the rule.

**How to apply:** Any time the user flags a missed memory step, commit, or process failure —
read `tasks/lessons.md` first, find the relevant rule, then act on its full text.

## 2026-04-17 — Subphase branches must use version numbers, not phase/N.M labels

**Rule:** Subphase implementation branches must be named by version number only: `0.x.y`
(e.g. `0.1.1`). No description suffix. The branch name matches the `v0.x.y` PATCH tag
the subphase contributes to.

**Why:** Branch names should be version identifiers, consistent with the `v0.x.y` release
tagging scheme in `docs/VERSIONING.md`. No description suffix needed.

**How to apply:** When starting any new subphase, branch from `0.x` using the plain
version: `git checkout -b 0.1.2 0.1`. The old `phase/N.M-description` pattern is
replaced going forward.

## 2026-04-17 — All builds via docker buildx only; never run local cargo/npm

**Rule:** All build verification must happen inside Docker using `docker buildx build`. Never run
`cargo build`, `npm build`, or any other local build tool directly.

**Why:** Docker is the canonical build environment. Local toolchain differences can mask build
failures that Docker would catch.

**How to apply:** Skip any plan step that calls `cargo build` or similar local commands. Move
verification to the Dockerfile build step using `docker buildx build --progress=plain -t <tag> .`.

## 2026-04-17 — Pause after each plan task; do not batch tasks

**Rule:** After completing and committing each numbered plan task, report what was done and wait
for explicit approval before continuing to the next task.

**Why:** User wants a review checkpoint at every task boundary, not after groups of three.

**How to apply:** Execute one task completely (all steps + commit), report, then stop. The
executing-plans skill's default of "3 tasks per batch" is overridden by this rule.

## 2026-04-18 — No per-subphase branches; work directly on the phase branch

**Context:** Phase 2 work was being done on `0.2.1`, a subphase branch cut from `0.2`.
User asked to merge up and drop the subphase branch convention.

**Rule:** Do all phase development directly on the phase branch (e.g. `0.2`). Do not create
numbered subphase branches (e.g. `0.2.1`, `0.2.2`). The CLAUDE.md branching section describes
subphase branches as an option; this project does not use them.

**Why:** Extra branch granularity adds merge overhead without benefit for a solo-operator project.

**How to apply:** When starting a new phase, create only the phase branch (e.g. `git checkout -b 0.3 main`).
Commit subphase work directly to it. No subphase branch needed.

## 2026-04-20 — Present plan before implementing — third reminder

**Mistake:** User described bug fixes. Code was written and committed before any plan or
diagnosis was presented for review. This is the third time this pattern has recurred across
sessions despite two prior entries in this file (2026-04-11, 2026-04-15).

**Rule:** No code, no file edits, no agent dispatches until a plan has been presented and the
user has explicitly approved it. This applies to:
- Bug fixes, even "obvious" ones
- Any request involving more than a trivial one-liner
- Cases where the user says "fix these" or lists issues — that is a request to plan, not execute

**Why:** The user has flagged this more than once. Every recurrence adds correction overhead
and erodes trust in the workflow. The rule exists precisely because the temptation to
"just fix it" is strong and consistently wrong.

**How to apply:** When a user describes problems or requests changes:
1. Output a structured diagnosis and proposed approach (what changes, which files, why)
2. End the response — do not write any code
3. Wait for explicit approval before touching any file
Use `/write-plan` and `/execute-plan` skills. The plan-then-approve gate is not optional.

## 2026-04-16 — Commit all documentation changes immediately, not just plan docs

**Pattern:** CHANGELOG.md was updated but not committed. The existing rule in memory was scoped
to plan docs only, missing the broader intent.

**Rule:** Any documentation change — CHANGELOG.md, release notes, plan docs in `docs/plans/`,
`tasks/lessons.md` updates — must be committed immediately after writing. Never leave doc
changes as uncommitted working-tree modifications.

**Why:** Docs are project record. Untracked or unstaged doc changes won't appear in history and
can be lost.

**How to apply:** After any doc-only edit, `git add <file> && git commit` before doing anything
else. The rule isn't limited to plan docs — it applies to any file whose purpose is documentation.

## 2026-04-21 — Present plan before implementing — fourth reminder

**Mistake:** User reported two bugs (scan 405, jobs tab showing wrong page). Before presenting
any diagnosis or plan, code was written across four files and the response ended with a summary
of what was done. This is the fourth recurrence of this pattern (2026-04-11, 2026-04-15,
2026-04-20, 2026-04-21). Additionally, changes were not committed and feedback was not captured.

**Rule:** When a user describes bugs or issues, the only correct first response is a structured
diagnosis and proposed fix plan. Stop after presenting the plan. Do not touch any file. Do not
write any code. Do not dispatch any agent. Wait for explicit approval.

**Why:** Four separate log entries have not broken this habit. The temptation to "just fix it"
overrides the rule when the fix seems straightforward. That temptation must be treated as a
signal to slow down, not speed up.

**Also captured this session:**
- Not committing changes after implementation violates the 2026-04-04 rule ("Always commit
  changes at end of session") and the 2026-04-10 rule ("Commit at batch boundaries").
- Not saving feedback to memory violates the 2026-04-05 rule ("Every memory write requires a
  paired tasks/lessons.md commit") and the 2026-04-21 rule ("Capture session feedback in
  docs/summaries/").

**How to apply:**
1. User describes issues → output diagnosis + file-by-file plan → stop → wait for approval
2. After implementation → commit → capture feedback in memory + lessons.md + docs/summaries/

## 2026-04-21 — Capture session feedback in docs/summaries/ files

**Rule:** At the end of any significant implementation session, write a summary file to
`docs/summaries/YYYY-MM-DD-<topic>.md` using the project's date-prefixed kebab-case convention.

The summary must include:
- What was implemented (per batch/task)
- Key decisions and rationale
- A "Feedback Captured" section listing corrections, notable approvals, and guidance from the user

**Why:** Memory files are machine-local. `docs/summaries/` is git-tracked and portable — the
feedback record travels with the repo and is available to all contributors and future sessions.

**How to apply:**
- When a user provides feedback mid-session (corrections, approvals of non-obvious choices,
  strong preferences), note it in the Feedback Captured section — not only in memory files
- **Iteration feedback** (user clears context and provides feedback on a build, reports a bug,
  reacts to a deployed change): append to an existing summary for that topic, or create a new
  one named `YYYY-MM-DD-<topic>-feedback.md` — do not wait until end of session
- Commit the summary file immediately after writing — not deferred to end of session
- If no summary exists yet for the topic, create one even if the only content is the feedback section

## 2026-04-21 — Never reuse a previous phase branch number

**Rule:** Phase branches use plain version numbers: `0.x` (e.g., `0.6`). Before creating a
new phase branch, run `git branch -a` to find the highest existing `0.x` branch and use the
next number. Never guess a low number like `0.3` if higher ones have already been used.

**Why:** User corrected `0.3` → `0.6`, confirming phases 0.1–0.5 were already used. Reusing
a number would collide with existing history.

**How to apply:** `git branch -a | grep -E '^\s*(remotes/origin/)?0\.' | sort -V` — pick
`max + 1`.

## 2026-04-21 — Write session summaries inline during work, not at end of session

**Mistake:** Summary files were not written during the session. The user had to explicitly prompt
for them to be created — twice across sessions.

**Rule:** Write `docs/summaries/` entries during the work, not after. The user must never have
to ask for a summary to be created.

**How to apply:**
- After each significant task is committed, append to the running session summary (or create it
  if it doesn't exist yet for this topic)
- When the user provides feedback mid-session (correction, approval, clarification), write it to
  the summary immediately — do not batch it
- Commit the summary update in the same commit as the task, or immediately after
- The summary file is a running log that grows as work progresses, not a final artifact written
  at the end

## 2026-04-24 — User input gets highest priority; never defer mid-session messages

**Rule:** User messages sent mid-task must be read and addressed immediately. They interrupt
whatever is in progress. The only exception: the user has explicitly said to queue or defer
(e.g., "finish what you're doing first").

**Why:** User explicitly stated this rule and required it to survive compaction. Two violations
occurred in a single session: (1) a compaction-injected "IMPORTANT: address after current task"
was ignored while coding continued; (2) a second user message ("you're ignoring my input") was
also deferred. Both are the same failure.

**How to apply:**
- When a user message arrives mid-task, stop and respond to it before resuming
- "IMPORTANT: address user message after current task" (a compaction artifact) does NOT mean
  the current task takes priority — treat it as "respond now"
- User messages may be queued or deferred only if the user explicitly says so

**Applied:** All sessions. This survives compaction by being in `tasks/lessons.md`.

## 2026-04-23 — No plan doc when user says to go straight to implementation after brainstorming

**Mistake:** After a completed brainstorming session where the user approved each design section
and said "take it straight to implementation," a plan document was written before touching code.

**Rule:** When the user says "take it straight to implementation" (or equivalent) after a
brainstorming session, skip the plan doc entirely and write the code directly.

**Why:** The brainstorming session is the plan. Writing a separate plan doc re-processes settled
decisions, introduces gaps between the agreed design and the implementation, and adds overhead
without value. The user called this out explicitly.

**How to apply:** The "commit plan before implementing" rule does NOT apply when the user has
explicitly said to skip to implementation after a completed brainstorm. Implement directly.

## 2026-04-24 — Skip writing-plans after brainstorming; implement from spec directly

**Rule:** After a completed brainstorming session that produced an approved spec, skip the
`writing-plans` skill entirely and implement directly from the spec document.

**Why:** The user has observed that `writing-plans` routinely leaves gaps between the design
spec and the implementation plan — it re-processes already-settled decisions without adding
value. The spec is the plan.

**How to apply:** When the user says "skip straight to implementation" after approving a
brainstorm spec, do NOT invoke `writing-plans`. Implement directly from the spec, using its
sections as the task sequence. The spec commit already satisfies the "commit plan first" rule.

## 2026-04-24 — All plans and specs go in docs/plans/, not docs/superpowers/specs/

**Rule:** The brainstorming skill's default spec path (`docs/superpowers/specs/`) is overridden
by this project's convention. All plans and design specs belong in `docs/plans/` using the
established `YYYY-MM-DD-kebab-slug.md` naming pattern.

**Why:** User corrected placement mid-session. The project has a single canonical location for
all planning documents regardless of which skill generated them.

**How to apply:** When writing any spec or plan doc, always use `docs/plans/` as the target
directory. Check sibling files for the naming convention before creating the file.

## 2026-04-25 — Present plan before implementing — fifth reminder

**Mistake:** User reported a failing MB lookup (400 Bad Request). The bug was diagnosed and the
fix was applied immediately — without presenting the diagnosis or waiting for approval. This is
the fifth recurrence (2026-04-11, 2026-04-15, 2026-04-20, 2026-04-21, 2026-04-25).

**Rule:** No code, no file edits, no agent dispatches until a plan has been presented and the
user has explicitly approved it. Applies to bug fixes, even single-line ones.

**Why:** The temptation to "just fix it" is strongest for small, obvious bugs. That temptation
must be treated as a signal to slow down, not permission to skip the gate.

**How to apply:**
1. Diagnose the issue — output the root cause and the proposed change (file, line, what changes)
2. End the response — do not touch any file
3. Wait for explicit approval before writing a single character of code
