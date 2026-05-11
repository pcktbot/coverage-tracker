---
name: restore-first
description: Use when the user says a file, directory, function, or config was deleted, removed, missing, lost, or gone. BEFORE regenerating from scratch, recover from git history.
---

# Restore First

The user said something is missing. **Do not regenerate from scratch
yet.** The recurring failure mode is generating a plausible-looking
replacement that drifts from the prior content — losing migrations,
edge-case handling, comments, or exact config the user expected to
recover.

## Two-step recovery

### 1. Find when it existed

```
git log --all --oneline -- <path>
```

Use `--all` so deleted branches, stashes, and merge ancestors are
searched. If the user gave a directory, run it on the directory. If
they gave a filename without a path, try both `git log --all --oneline
-- "*/<name>"` and `git log --all --oneline --follow -- <name>`.

If no results, also try:
- A broader path (parent directory).
- The filename in `git stash list`.
- `git fsck --lost-found` for orphaned blobs (only if nothing else
  works).

### 2. Recover the content

Once you have a commit sha, view or restore the file:

```
git show <sha>:<path>          # view content
git checkout <sha> -- <path>   # restore into working tree
```

Pick the **latest** sha where the file existed in a known-good state.
If the user mentioned roughly when it was deleted ("last week", "in
the rename commit"), use that to choose between candidates. Otherwise
default to the most recent.

## Report back before editing

After recovery, tell the user:

- The commit sha you recovered from and its commit message.
- The size / line count of the recovered file.
- Whether you've placed it back in the working tree or are only
  showing it.

Wait for confirmation before making further changes built on top.

## When to skip

- The user explicitly says "nothing exists in history" or "I never
  committed it" — generate from scratch, but mention you confirmed
  with `git log --all` (one command, no harm).
- The "deleted" item is a single line / small block, not a file or
  directory — just restore from context, no git archaeology needed.
- The user is asking to delete something — opposite intent, this
  skill does not apply.

## Why

Past sessions regenerated Helm charts, migrations, and config files
from scratch when git history had the exact prior state. Regeneration
loses domain knowledge that was encoded but undocumented in the
original. `git log --all` is two seconds; regeneration is minutes and
often wrong.
