---
name: parallel-repo-explorer
description: Use when the user's task spans 2+ repositories (e.g., "across the onboarder and CMS", "in all three repos", "multi-repo feature"). Fan out to parallel Agent calls — one per repo — with a fixed report shape, then synthesize before proposing changes.
---

# Parallel Repo Explorer

The task touches multiple repos. Sequential exploration is slow and
biases synthesis toward whichever repo you read first. The fix is
deliberate parallel fan-out.

## Procedure

### 1. Identify the repos

Confirm with the user (one line) which repos are in scope if it's
ambiguous. Don't guess silently. Don't proceed until each repo has a
concrete path or remote.

### 2. Spawn one agent per repo, in parallel

Use the `Explore` agent (read-only, fast) for each repo. **All Agent
tool calls must be in a single message** so they run concurrently —
sequential calls defeat the purpose of this skill.

Each agent gets the same prompt template, parameterized by repo:

> Explore `<repo-path>` for the task: `<one-sentence task description>`.
> Report back in this exact shape, under 300 words:
>
> - **Relevant files** (paths + 1-line role each)
> - **Current behavior** — what the code does today for this concern
> - **Integration points** — where this repo exposes/consumes
>   contracts with other repos in scope: APIs, message schemas,
>   shared types, env vars, file paths, ports
> - **Open questions** — anything ambiguous that synthesis will need

### 3. Synthesize before proposing changes

After all agents return, produce a single combined summary with these
sections:

- **Per-repo summary** (one paragraph each)
- **Shared contracts** — the integration points named by 2+ agents,
  reconciled
- **Disagreements / gaps** — places where the agents' findings
  conflict or one didn't surface info another did
- **Proposed approach** — only after the above is on the page

**Do not edit any file** until the synthesis is in front of the user
and they've agreed on the approach.

## When to skip

- Task touches only one repo (even if other repos are mentioned for
  context).
- Task is a trivial cross-repo grep (a single string lookup) — just
  grep directly.
- User has already given you a per-repo plan and asked you to execute
  it.

## Why

When you read repos sequentially you anchor on the first one's
vocabulary and miss contract mismatches. Fan-out forces each repo to
report in the same shape, which makes mismatches visible at synthesis
time instead of three edits in.
