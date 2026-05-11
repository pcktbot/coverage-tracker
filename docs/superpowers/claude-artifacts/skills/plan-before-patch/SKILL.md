---
name: plan-before-patch
description: Use when the user reports a bug, test failure, crash, exception, regression, or any "X doesn't work / X is broken / fix X" request, BEFORE making any code edits. Forces a written diagnosis with ranked hypotheses and falsifier before touching code.
---

# Plan Before Patch

You have been invoked because the user reported a defect. **Do not edit
code yet.** Recurring failure mode: jumping to a fix at the symptom site
when the actual cause is upstream. The remedy is a short written
diagnosis the user can confirm or redirect.

## What to produce, in this exact shape

Output a single message with these four sections — no more, no less:

### 1. Observed symptom
One or two sentences. Quote the user's report and the concrete failure
signal (error string, failing assertion, screenshot description, log
line). If the user didn't give one, ask for it and stop — do not
proceed without a signal.

### 2. Candidate root causes (2–3, ranked)
Rank by likelihood. For each: one sentence naming the cause, one
sentence on why it's plausible given the signal. Cover at least one
**upstream** cause (config, data, prior layer) — not just the obvious
display-side cause.

### 3. Evidence for the top hypothesis
Concrete `path/to/file.ext:line` references that support hypothesis #1.
If you have not read the code yet, say so and list which files you
need to read to confirm. Then read them. Do not skip this step.

### 4. Falsifier
One sentence: "If hypothesis #1 is wrong, I'd expect to see ___ when I
check ___." This is the thing you'd check to disprove yourself.

## Then stop

End the message with: **"Confirm the top hypothesis or redirect, and
I'll proceed."**

Wait for the user. Do not call Edit, Write, or any code-mutating tool
until they confirm.

## When to skip

- The user has already given you a root-cause diagnosis and asked you to
  implement a specific change.
- The "bug" is a one-line typo with a stack-trace line number — fix
  directly, but note in your reply you're skipping diagnosis because the
  cause is unambiguous.
- The user explicitly says "just try the obvious fix first."

In all other cases: diagnose first.

## Why

This skill exists because past sessions repeatedly patched display
controllers when the bug was in section-to-package mapping, patched the
auth callback when the bug was OIDC gem version skew, etc. The cost of
writing four short paragraphs is small; the cost of an edit at the
wrong layer is large.
