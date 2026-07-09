# Agent Dispatch Pipeline — Design Spec

**Date:** 2026-05-19  
**Status:** Draft  
**Scope:** Automated work item → agent execution pipeline with eval layer

---

## Problem

Today the workflow from "work item is ready" to "agent is working on it" is manual: find the item, start a Claude session, pull business artifacts, bootstrap the workflow. This spec defines an automated pipeline that closes that loop end-to-end, with an eval layer that improves quality over time.

---

## Goals

- Automatically detect work items entering a "ready-for-dev" state on the ADO Kanban board
- Orchestrate the full dev workflow (ingest → discovery → spec → impl plan → execution → PR → CI watch) as a durable, resumable process
- Gate phase transitions with AI judge activities scored against versioned rubrics
- Surface pipeline state, eval scores, and human-review controls in coverage-manager
- Build the eval result store as the foundation for a future comparative judge layer

## Non-Goals

- Multi-user or cloud sync (single-user, local-first for now)
- Mutating ADO work items beyond status updates
- Fine-tuning models (judges use prompted Claude SDK calls only)
- Production deployment (dev/staging environments only)

---

## System Boundaries

### `pipeline-service` (new submodule repo)

A standalone service that owns all pipeline logic. Added to coverage-manager as a git submodule at `./pipeline-service`.

**Contains:**
- Temporal worker + all workflow and activity definitions
- Claude SDK phase activities (multi-turn, one per workflow phase)
- Eval judge activities + versioned rubric configs (`rubrics/`)
- Git worktree lifecycle management
- Docker Compose stack: `temporal`, `postgresql`, `pipeline-worker`
- Small HTTP API (`localhost:9877`) for coverage-manager integration

**Deploys identically** local (Docker Compose) → cluster (same image, external Temporal + Postgres).

### `coverage-manager` (this repo)

Observability and control plane only. No workflow logic lives here.

**Changes:**
- HTTP client calls to `pipeline-service` API
- New `/pipeline` route (workflow-level view)
- `workflow_phase_changed` and `eval_result` event types in orchestrator DB
- "Open Temporal UI" button linking to `http://localhost:8233`

---

## Docker Compose Stack

```yaml
services:
  postgresql:       # Temporal backend + eval result store
  temporal:         # Temporal server (port 7233)
  temporal-ui:      # Temporal web UI (port 8233)
  pipeline-worker:  # The pipeline-service binary
```

No external dependencies for local dev. Temporal UI available at `http://localhost:8233`.

---

## Temporal Workflow Design

### `KanbanWatcherWorkflow`

- Runs on a schedule (configurable, default 5 min)
- Polls ADO for items entering the "ready-for-dev" column
- For each item: starts a `WorkItemWorkflow` with the work item ID as the workflow ID (idempotent — duplicate signals are no-ops)

### `WorkItemWorkflow`

One workflow instance per work item. Phases execute as sequential Temporal activities:

```
[Ingest] → [Discovery] → [Spec] → ⚖️ EvalGate(spec)
                                         ↓ pass
                                   [ImplPlan] → ⚖️ EvalGate(impl-plan)
                                                       ↓ pass
                                               [Execution] → [PR] → [CIWatch]
```

**Phase activities (Claude SDK):**
- `IngestActivity` — fetches ADO work item, linked Confluence docs, repo context; produces structured `WorkItemContext`
- `DiscoveryActivity` — multi-turn Claude self-directed analysis of the work item context; identifies unknowns and resolves ambiguities from available artifacts (ADO, Confluence) without human interaction; unresolvable unknowns are recorded in the output for the spec phase to surface
- `SpecActivity` — produces a written spec document from discovery output
- `ImplPlanActivity` — produces a step-by-step implementation plan from the spec
- `ExecutionActivity` — drives implementation in an isolated git worktree (see Worktree section)
- `PRActivity` — creates GitHub PR with spec + impl plan in description
- `CIWatchActivity` — polls CI status, surfaces failures back to the workflow

**Worktree management:**
- `WorktreeActivity` creates an isolated branch + worktree before `ExecutionActivity`
- Cleaned up after `PRActivity` completes
- Branch name: `pipeline/{work-item-id}`

### Failure & Retry Model

**Eval gate fail:**
1. Judge feedback string is injected into the next Claude turn as a correction prompt
2. Phase activity retries (up to 3 attempts)
3. After 3 failures: workflow pauses, fires `human_review_required` event to coverage-manager
4. Resumes on `approve` or `reject` Temporal signal from the UI

**Activity errors:** Standard Temporal retry policy with exponential backoff. Non-retryable errors (e.g., ADO auth failure) surface immediately as workflow errors.

---

## Eval Layer

### Judge Activity

A separate Claude SDK call (not the phase agent) that receives phase output + rubric and returns a structured score.

**Input:**
```typescript
{
  phase: "spec" | "impl-plan",
  content: string,        // phase output text
  workItemContext: WorkItemContext,
  rubricVersion: string,
  priorAttempts: JudgeResult[],  // for retry context
}
```

**Output:**
```typescript
{
  overall: number,        // 0.0–1.0
  pass: boolean,
  criteria: { id, score, rationale }[],
  feedback: string,       // injected into retry turn if fail
  rubricVersion: string,
}
```

### Rubric Format

Versioned YAML files in `pipeline-service/rubrics/`:

```yaml
# rubrics/spec-v1.yaml
name: spec-v1
criteria:
  - id: completeness
    weight: 0.30
    prompt: "Does the spec cover all acceptance criteria from the work item?"
  - id: testability
    weight: 0.25
    prompt: "Can each requirement be verified by a test?"
  - id: scope
    weight: 0.25
    prompt: "Is scope bounded with no implicit or unbounded dependencies?"
  - id: ambiguity
    weight: 0.20
    prompt: "Are requirements unambiguous — no requirement interpretable two ways?"
pass_threshold: 0.75
```

### Eval Result Storage

Every judge result written to `eval_results` in Postgres:

```sql
CREATE TABLE eval_results (
  id            UUID PRIMARY KEY,
  workflow_id   TEXT NOT NULL,
  work_item_id  TEXT NOT NULL,
  phase         TEXT NOT NULL,
  attempt       INTEGER NOT NULL,
  rubric_version TEXT NOT NULL,
  overall_score FLOAT NOT NULL,
  pass          BOOLEAN NOT NULL,
  criteria      JSONB NOT NULL,   -- per-criterion scores + rationale
  feedback      TEXT,
  outcome_label TEXT,             -- null until implementation outcome is known
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

`outcome_label` is set after implementation completes (success / difficult / failed). This is the seed of the comparative layer — labeled rows become examples for future judges.

### Future: Comparative Layer

When example set is large enough:
1. Add `pgvector` extension to the same Postgres instance
2. Add `embedding` column to `eval_results`
3. Judge prompt includes top-N semantically similar past specs with their outcome labels
4. No new infrastructure — same DB, same judge activity, richer prompt

---

## Pipeline-Service HTTP API

Exposed on `localhost:9877`:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/workflows` | List runs with phase + eval summary |
| `GET` | `/workflows/:id` | Full run detail (phases, eval scores, retries) |
| `POST` | `/workflows/:id/signal` | Send Temporal signal (`approve`, `reject`, `retry`) |
| `POST` | `/workflows` | Manually trigger a run for a work item ID |
| `GET` | `/health` | Liveness check |

---

## Coverage-Manager Integration

### Event flow (existing plumbing, no changes)

Pipeline-service POSTs to coverage-manager's existing `POST /event` endpoint. Pipeline-spawned Claude sessions appear in `/inbox` automatically with full tool timeline and loaded snapshot.

Two new event types added to the orchestrator schema:
- `workflow_phase_changed` — `{ workflow_id, phase, status, eval_score? }`
- `eval_result` — `{ workflow_id, phase, attempt, overall, pass, criteria }`

### New `/pipeline` route

**Run list** — one card per work item:
- Title, ADO link, current phase, elapsed time
- Eval gate chips (pass/fail per gate, expandable to per-criterion breakdown)
- Paused/needs-input indicator with signal buttons

**Run detail drawer:**
- Phase timeline with timestamps
- Eval score breakdown per phase (criteria scores + judge rationale)
- Retry history with injected feedback strings
- Links to associated `/inbox` sessions

**Signal panel** (shown when workflow is paused):
- Approve / Reject / Retry buttons
- Optional text input to inject additional context before unblocking

**Trigger panel:**
- Manual work item ID input or ADO picker to kick off a run

**"Open Temporal UI" button** — calls Tauri `open("http://localhost:8233")` for full workflow debugging.

---

## Technology Choices

| Concern | Choice | Rationale |
|---------|--------|-----------|
| Workflow engine | Temporal | Durable execution, built-in retry, human-in-the-loop signals, strong TypeScript SDK |
| Pipeline-service language | TypeScript/Node | Temporal TS SDK is mature; Claude SDK is first-class TS; faster iteration than Rust for this layer |
| Eval judge | Claude SDK (claude-sonnet-4-6) | Structured output, cost-effective for rubric scoring |
| Phase agents | Claude SDK (claude-opus-4-7) | More capable for multi-turn planning/implementation |
| Example store | Postgres + pgvector (future) | No new infra; pgvector extension enables RAG when needed |
| Container orchestration | Docker Compose (local) | Single `docker compose up` for full stack including Temporal UI |

---

## Open Questions

1. Which ADO column name(s) map to "ready-for-dev"? (configurable in pipeline-service env)
2. Should the Kanban watcher assign the ADO item to a pipeline bot user when a workflow starts?
3. What is the initial set of work item types to support? (Feature, Bug, Task, or all?)
4. Should CI watch trigger a re-execution activity on failure, or just surface the failure and pause?
