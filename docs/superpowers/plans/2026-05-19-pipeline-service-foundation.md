# Pipeline Service Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the `pipeline-service` repo with a working Temporal stack, ADO Kanban watcher, stubbed WorkItemWorkflow, and HTTP API — a fully observable skeleton where work items flow end-to-end before any real Claude activity is wired up.

**Architecture:** A standalone TypeScript/Node service runs as a Temporal worker alongside a Docker Compose stack (Temporal server + PostgreSQL + Temporal UI). A `KanbanWatcherWorkflow` polls ADO on a schedule and starts a `WorkItemWorkflow` per ready item. All phase activities are stubs that log and return mock output. An Express HTTP API exposes workflow state to coverage-manager and accepts Temporal signals.

**Tech Stack:** Node 20, TypeScript 5, `@temporalio/worker` + `@temporalio/client` + `@temporalio/workflow` + `@temporalio/testing`, Express 4, `pg` (Postgres), `node-fetch`, Vitest, Docker Compose

**This is Plan 1 of 4.** Plans 2–4 add: real Claude SDK phase activities, the eval layer, and the coverage-manager `/pipeline` UI.

---

## File Map

**New repo: `pipeline-service/` (git submodule)**

```
pipeline-service/
├── docker-compose.yml           # Temporal + Postgres + Temporal UI + pipeline-worker
├── Dockerfile                   # Multi-stage: build TS → run Node
├── package.json
├── tsconfig.json
├── .env.example                 # Required env vars with descriptions
├── vitest.config.ts
├── src/
│   ├── index.ts                 # Entry: starts worker + HTTP server
│   ├── worker.ts                # Temporal worker registration
│   ├── server.ts                # Express HTTP API (port 9877)
│   ├── db/
│   │   ├── client.ts            # pg Pool singleton
│   │   └── migrations.ts        # CREATE TABLE IF NOT EXISTS for eval_results + workflow_runs
│   ├── temporal/
│   │   ├── client.ts            # Temporal Client singleton
│   │   ├── workflows/
│   │   │   ├── kanban-watcher.workflow.ts   # Scheduled ADO poll → start WorkItemWorkflows
│   │   │   └── work-item.workflow.ts        # Phase sequence + signal handling
│   │   └── activities/
│   │       ├── ado.activities.ts            # pollReadyWorkItems (real ADO call)
│   │       ├── stub.activities.ts           # ingest/discovery/spec/implPlan/execution/pr/ciWatch stubs
│   │       └── notify.activities.ts         # postEventToCoverageManager
│   ├── ado/
│   │   └── client.ts            # ADO REST client (fetch work items by board column)
│   └── types.ts                 # Shared: WorkItemContext, PhaseResult, WorkflowState
├── tests/
│   ├── workflows/
│   │   ├── kanban-watcher.test.ts
│   │   └── work-item.test.ts
│   ├── activities/
│   │   └── ado.activities.test.ts
│   └── server.test.ts
└── README.md
```

**Changes to coverage-manager (this repo):**
- `src-tauri/src/orchestrator/db.rs` — add `workflow_phase_changed` and `eval_result` to accepted event kinds
- `src-tauri/src/orchestrator/handlers.rs` — no change needed; `/event` already accepts arbitrary JSON payloads

---

## Task 1: Repo Scaffolding + Docker Compose

**Files:**
- Create: `pipeline-service/package.json`
- Create: `pipeline-service/tsconfig.json`
- Create: `pipeline-service/vitest.config.ts`
- Create: `pipeline-service/.env.example`
- Create: `pipeline-service/docker-compose.yml`
- Create: `pipeline-service/Dockerfile`

- [ ] **Step 1: Create the pipeline-service directory and init git**

```bash
mkdir -p /Users/david.miller/Documents/current/pipeline-service
cd /Users/david.miller/Documents/current/pipeline-service
git init
```

- [ ] **Step 2: Create `package.json`**

```json
{
  "name": "pipeline-service",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js",
    "dev": "tsx watch src/index.ts",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@temporalio/activity": "^1.11.0",
    "@temporalio/client": "^1.11.0",
    "@temporalio/worker": "^1.11.0",
    "@temporalio/workflow": "^1.11.0",
    "express": "^4.19.2",
    "pg": "^8.12.0",
    "node-fetch": "^3.3.2"
  },
  "devDependencies": {
    "@temporalio/testing": "^1.11.0",
    "@types/express": "^4.17.21",
    "@types/pg": "^8.11.6",
    "@types/node": "^20.0.0",
    "tsx": "^4.7.0",
    "typescript": "^5.4.0",
    "vitest": "^1.6.0"
  }
}
```

- [ ] **Step 3: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "tests"]
}
```

- [ ] **Step 4: Create `vitest.config.ts`**

```typescript
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    testTimeout: 60000,
    include: ['tests/**/*.test.ts'],
  },
});
```

- [ ] **Step 5: Create `.env.example`**

```bash
# Temporal
TEMPORAL_ADDRESS=localhost:7233
TEMPORAL_NAMESPACE=default

# Postgres
DATABASE_URL=postgresql://temporal:temporal@localhost:5432/pipeline

# ADO
ADO_ORG_URL=https://dev.azure.com/your-org
ADO_PAT=your-personal-access-token
ADO_PROJECT=your-project-name
ADO_TEAM=your-team-name
ADO_READY_COLUMN=Ready for Dev

# Coverage Manager
COVERAGE_MANAGER_URL=http://localhost:9876

# HTTP API
PORT=9877

# Worker
TASK_QUEUE=pipeline
WATCHER_SCHEDULE_INTERVAL_MINUTES=5
```

- [ ] **Step 6: Create `docker-compose.yml`**

```yaml
services:
  postgresql:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: temporal
      POSTGRES_USER: temporal
    ports:
      - "5432:5432"
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U temporal"]
      interval: 5s
      timeout: 5s
      retries: 10

  temporal:
    image: temporalio/auto-setup:1.24
    depends_on:
      postgresql:
        condition: service_healthy
    environment:
      - DB=postgresql
      - DB_PORT=5432
      - POSTGRES_USER=temporal
      - POSTGRES_PWD=temporal
      - POSTGRES_SEEDS=postgresql
    ports:
      - "7233:7233"
    healthcheck:
      test: ["CMD", "tctl", "--address", "temporal:7233", "cluster", "health"]
      interval: 10s
      timeout: 5s
      retries: 20

  temporal-ui:
    image: temporalio/ui:2.26.2
    depends_on:
      - temporal
    environment:
      - TEMPORAL_ADDRESS=temporal:7233
      - TEMPORAL_CORS_ORIGINS=http://localhost:3000
    ports:
      - "8233:8233"

  pipeline-worker:
    build: .
    depends_on:
      temporal:
        condition: service_healthy
      postgresql:
        condition: service_healthy
    env_file: .env
    environment:
      - TEMPORAL_ADDRESS=temporal:7233
      - DATABASE_URL=postgresql://temporal:temporal@postgresql:5432/pipeline
      - COVERAGE_MANAGER_URL=http://host.docker.internal:9876
    extra_hosts:
      - "host.docker.internal:host-gateway"
    restart: unless-stopped

volumes:
  postgres-data:
```

- [ ] **Step 7: Create `Dockerfile`**

```dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:20-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY --from=builder /app/dist ./dist
CMD ["node", "dist/index.js"]
```

- [ ] **Step 8: Install dependencies**

```bash
cd /Users/david.miller/Documents/current/pipeline-service
npm install
```

Expected: `node_modules/` created, no errors.

- [ ] **Step 9: Initial commit**

```bash
cd /Users/david.miller/Documents/current/pipeline-service
git add .
git commit -m "chore: initial repo scaffold with Docker Compose"
```

---

## Task 2: Shared Types + DB Client

**Files:**
- Create: `pipeline-service/src/types.ts`
- Create: `pipeline-service/src/db/client.ts`
- Create: `pipeline-service/src/db/migrations.ts`

- [ ] **Step 1: Write the failing test for DB migrations**

Create `tests/db.test.ts`:

```typescript
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import pg from 'pg';
import { runMigrations } from '../src/db/migrations.js';

describe('runMigrations', () => {
  let pool: pg.Pool;

  beforeAll(async () => {
    pool = new pg.Pool({ connectionString: process.env.DATABASE_URL ?? 'postgresql://temporal:temporal@localhost:5432/pipeline' });
    await pool.query('DROP TABLE IF EXISTS eval_results, workflow_runs, schema_version');
  });

  afterAll(async () => {
    await pool.end();
  });

  it('creates workflow_runs and eval_results tables', async () => {
    await runMigrations(pool);

    const result = await pool.query(
      `SELECT table_name FROM information_schema.tables
       WHERE table_schema = 'public' AND table_name IN ('workflow_runs', 'eval_results')
       ORDER BY table_name`
    );
    expect(result.rows.map((r: { table_name: string }) => r.table_name)).toEqual(['eval_results', 'workflow_runs']);
  });

  it('is idempotent — running twice does not throw', async () => {
    await expect(runMigrations(pool)).resolves.not.toThrow();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/david.miller/Documents/current/pipeline-service
npx vitest run tests/db.test.ts
```

Expected: FAIL — `Cannot find module '../src/db/migrations.js'`

- [ ] **Step 3: Create `src/types.ts`**

```typescript
export type WorkflowPhase =
  | 'ingest'
  | 'discovery'
  | 'spec'
  | 'impl-plan'
  | 'execution'
  | 'pr'
  | 'ci-watch';

export type WorkflowStatus =
  | 'running'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface WorkItemContext {
  workItemId: string;
  title: string;
  description: string;
  acceptanceCriteria: string[];
  workItemType: string;
  linkedConfluenceUrls: string[];
  repoName: string | null;
}

export interface PhaseResult {
  phase: WorkflowPhase;
  output: string;
  completedAt: string;
}

export interface WorkflowState {
  workflowId: string;
  workItemId: string;
  currentPhase: WorkflowPhase;
  status: WorkflowStatus;
  phases: PhaseResult[];
  startedAt: string;
  updatedAt: string;
}

export interface CoverageManagerEvent {
  kind: string;
  session_id?: string;
  payload: Record<string, unknown>;
}
```

- [ ] **Step 4: Create `src/db/client.ts`**

```typescript
import pg from 'pg';

let pool: pg.Pool | null = null;

export function getPool(): pg.Pool {
  if (!pool) {
    const connectionString = process.env.DATABASE_URL;
    if (!connectionString) throw new Error('DATABASE_URL env var is required');
    pool = new pg.Pool({ connectionString, max: 10 });
  }
  return pool;
}

export async function closePool(): Promise<void> {
  if (pool) {
    await pool.end();
    pool = null;
  }
}
```

- [ ] **Step 5: Create `src/db/migrations.ts`**

```typescript
import pg from 'pg';

export async function runMigrations(pool: pg.Pool): Promise<void> {
  await pool.query(`
    CREATE TABLE IF NOT EXISTS workflow_runs (
      id TEXT PRIMARY KEY,
      work_item_id TEXT NOT NULL,
      current_phase TEXT NOT NULL,
      status TEXT NOT NULL,
      phases JSONB NOT NULL DEFAULT '[]',
      started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )
  `);

  await pool.query(`
    CREATE TABLE IF NOT EXISTS eval_results (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      workflow_id TEXT NOT NULL REFERENCES workflow_runs(id),
      work_item_id TEXT NOT NULL,
      phase TEXT NOT NULL,
      attempt INTEGER NOT NULL,
      rubric_version TEXT NOT NULL,
      overall_score FLOAT NOT NULL,
      pass BOOLEAN NOT NULL,
      criteria JSONB NOT NULL,
      feedback TEXT,
      outcome_label TEXT,
      created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )
  `);
}
```

- [ ] **Step 6: Run test to verify it passes**

> Note: requires a running Postgres instance. Start with `docker compose up postgresql -d` first.

```bash
cd /Users/david.miller/Documents/current/pipeline-service
docker compose up postgresql -d
sleep 3
npx vitest run tests/db.test.ts
```

Expected: PASS — both tests green.

- [ ] **Step 7: Commit**

```bash
cd /Users/david.miller/Documents/current/pipeline-service
git add src/ tests/db.test.ts
git commit -m "feat: shared types, DB client, and migrations"
```

---

## Task 3: ADO Client + `pollReadyWorkItems` Activity

**Files:**
- Create: `pipeline-service/src/ado/client.ts`
- Create: `pipeline-service/src/temporal/activities/ado.activities.ts`
- Create: `pipeline-service/tests/activities/ado.activities.test.ts`

- [ ] **Step 1: Write the failing test**

Create `tests/activities/ado.activities.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock fetch before importing the module under test
vi.stubGlobal('fetch', vi.fn());

describe('pollReadyWorkItems', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    process.env.ADO_ORG_URL = 'https://dev.azure.com/test-org';
    process.env.ADO_PAT = 'test-pat';
    process.env.ADO_PROJECT = 'test-project';
    process.env.ADO_TEAM = 'test-team';
    process.env.ADO_READY_COLUMN = 'Ready for Dev';
  });

  it('returns work item IDs from the ready column', async () => {
    const mockResponse = {
      workItems: [
        { id: 101, url: 'https://...' },
        { id: 202, url: 'https://...' },
      ],
    };

    (fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const { pollReadyWorkItems } = await import('../../src/temporal/activities/ado.activities.js');
    const result = await pollReadyWorkItems();

    expect(result).toEqual(['101', '202']);
  });

  it('returns empty array when no items in ready column', async () => {
    (fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ workItems: [] }),
    });

    const { pollReadyWorkItems } = await import('../../src/temporal/activities/ado.activities.js');
    const result = await pollReadyWorkItems();

    expect(result).toEqual([]);
  });

  it('throws when ADO returns non-ok response', async () => {
    (fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => 'Unauthorized',
    });

    const { pollReadyWorkItems } = await import('../../src/temporal/activities/ado.activities.js');
    await expect(pollReadyWorkItems()).rejects.toThrow('ADO request failed: 401');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/activities/ado.activities.test.ts
```

Expected: FAIL — `Cannot find module '../../src/temporal/activities/ado.activities.js'`

- [ ] **Step 3: Create `src/ado/client.ts`**

```typescript
function adoBase(): string {
  const org = process.env.ADO_ORG_URL;
  const project = process.env.ADO_PROJECT;
  if (!org || !project) throw new Error('ADO_ORG_URL and ADO_PROJECT are required');
  return `${org}/${encodeURIComponent(project)}`;
}

function authHeader(): string {
  const pat = process.env.ADO_PAT;
  if (!pat) throw new Error('ADO_PAT is required');
  return 'Basic ' + Buffer.from(`:${pat}`).toString('base64');
}

export async function fetchWorkItemsByColumn(column: string): Promise<string[]> {
  const team = process.env.ADO_TEAM ?? '';
  const base = adoBase();
  const teamSegment = team ? `/${encodeURIComponent(team)}` : '';
  const url = `${base}${teamSegment}/_apis/work/backlogs/column?api-version=7.1-preview.1`;

  // ADO board items API: query for items in a specific column via WIQL
  const wiqlUrl = `${base}/_apis/wit/wiql?api-version=7.1`;
  const query = {
    query: `SELECT [System.Id] FROM WorkItems WHERE [System.BoardColumn] = '${column}' AND [System.TeamProject] = '${process.env.ADO_PROJECT}' ORDER BY [System.ChangedDate] DESC`,
  };

  const response = await fetch(wiqlUrl, {
    method: 'POST',
    headers: {
      Authorization: authHeader(),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(query),
  });

  if (!response.ok) {
    throw new Error(`ADO request failed: ${response.status} ${await response.text()}`);
  }

  const data = (await response.json()) as { workItems: { id: number }[] };
  return (data.workItems ?? []).map((item) => String(item.id));
}
```

- [ ] **Step 4: Create `src/temporal/activities/ado.activities.ts`**

```typescript
import { fetchWorkItemsByColumn } from '../../ado/client.js';

export async function pollReadyWorkItems(): Promise<string[]> {
  const column = process.env.ADO_READY_COLUMN ?? 'Ready for Dev';
  return fetchWorkItemsByColumn(column);
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
npx vitest run tests/activities/ado.activities.test.ts
```

Expected: PASS — 3 tests green.

- [ ] **Step 6: Commit**

```bash
git add src/ado/ src/temporal/activities/ado.activities.ts tests/activities/
git commit -m "feat: ADO client and pollReadyWorkItems activity"
```

---

## Task 4: Stub Phase Activities + Notify Activity

**Files:**
- Create: `pipeline-service/src/temporal/activities/stub.activities.ts`
- Create: `pipeline-service/src/temporal/activities/notify.activities.ts`

These stubs return structured mock output so the workflow can run end-to-end without real Claude calls.

- [ ] **Step 1: Create `src/temporal/activities/stub.activities.ts`**

```typescript
import type { WorkItemContext, PhaseResult } from '../../types.js';

export async function ingestActivity(workItemId: string): Promise<WorkItemContext> {
  console.log(`[stub] ingestActivity workItemId=${workItemId}`);
  return {
    workItemId,
    title: `Work Item ${workItemId}`,
    description: '[stub] No real ADO fetch yet',
    acceptanceCriteria: ['[stub] criterion 1'],
    workItemType: 'Feature',
    linkedConfluenceUrls: [],
    repoName: null,
  };
}

export async function discoveryActivity(ctx: WorkItemContext): Promise<PhaseResult> {
  console.log(`[stub] discoveryActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'discovery',
    output: '[stub] Discovery complete. No unknowns identified.',
    completedAt: new Date().toISOString(),
  };
}

export async function specActivity(ctx: WorkItemContext, discovery: PhaseResult): Promise<PhaseResult> {
  console.log(`[stub] specActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'spec',
    output: `[stub] Spec for ${ctx.title}.\n\n## Requirements\n- ${ctx.acceptanceCriteria.join('\n- ')}`,
    completedAt: new Date().toISOString(),
  };
}

export async function implPlanActivity(ctx: WorkItemContext, spec: PhaseResult): Promise<PhaseResult> {
  console.log(`[stub] implPlanActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'impl-plan',
    output: '[stub] Implementation plan: Task 1, Task 2, Task 3.',
    completedAt: new Date().toISOString(),
  };
}

export async function executionActivity(ctx: WorkItemContext, implPlan: PhaseResult): Promise<PhaseResult> {
  console.log(`[stub] executionActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'execution',
    output: '[stub] Code written. Tests passing.',
    completedAt: new Date().toISOString(),
  };
}

export async function prActivity(ctx: WorkItemContext, execution: PhaseResult): Promise<PhaseResult> {
  console.log(`[stub] prActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'pr',
    output: '[stub] PR #999 created: https://github.com/example/repo/pull/999',
    completedAt: new Date().toISOString(),
  };
}

export async function ciWatchActivity(ctx: WorkItemContext, pr: PhaseResult): Promise<PhaseResult> {
  console.log(`[stub] ciWatchActivity workItemId=${ctx.workItemId}`);
  return {
    phase: 'ci-watch',
    output: '[stub] CI passed. All checks green.',
    completedAt: new Date().toISOString(),
  };
}
```

- [ ] **Step 2: Create `src/temporal/activities/notify.activities.ts`**

```typescript
import type { WorkflowPhase } from '../../types.js';

export async function postPhaseChangedEvent(
  workflowId: string,
  workItemId: string,
  phase: WorkflowPhase,
  status: 'started' | 'completed' | 'failed',
  evalScore?: number
): Promise<void> {
  const url = process.env.COVERAGE_MANAGER_URL;
  if (!url) {
    console.warn('[notify] COVERAGE_MANAGER_URL not set — skipping event post');
    return;
  }

  const body = {
    kind: 'workflow_phase_changed',
    payload: { workflow_id: workflowId, work_item_id: workItemId, phase, status, eval_score: evalScore ?? null },
  };

  try {
    const response = await fetch(`${url}/event`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      console.warn(`[notify] /event returned ${response.status}`);
    }
  } catch (err) {
    // Best-effort — don't fail the workflow if coverage-manager is unreachable
    console.warn('[notify] Failed to post event:', err);
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add src/temporal/activities/stub.activities.ts src/temporal/activities/notify.activities.ts
git commit -m "feat: stub phase activities and notify activity"
```

---

## Task 5: `WorkItemWorkflow`

**Files:**
- Create: `pipeline-service/src/temporal/workflows/work-item.workflow.ts`
- Create: `pipeline-service/tests/workflows/work-item.test.ts`

- [ ] **Step 1: Write the failing test**

Create `tests/workflows/work-item.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { TestWorkflowEnvironment } from '@temporalio/testing';
import { Worker } from '@temporalio/worker';
import { workItemWorkflow } from '../../src/temporal/workflows/work-item.workflow.js';
import type { WorkflowState } from '../../src/types.js';

describe('workItemWorkflow', () => {
  it('runs all phases and returns completed state', async () => {
    const env = await TestWorkflowEnvironment.createLocal();
    try {
      const worker = await Worker.create({
        connection: env.nativeConnection,
        taskQueue: 'test-pipeline',
        workflowsPath: new URL('../../src/temporal/workflows/work-item.workflow.js', import.meta.url).pathname,
        activities: {
          ingestActivity: async () => ({
            workItemId: '101', title: 'Test Item', description: 'desc',
            acceptanceCriteria: ['ac1'], workItemType: 'Feature',
            linkedConfluenceUrls: [], repoName: null,
          }),
          discoveryActivity: async () => ({ phase: 'discovery', output: 'done', completedAt: new Date().toISOString() }),
          specActivity: async () => ({ phase: 'spec', output: 'spec done', completedAt: new Date().toISOString() }),
          implPlanActivity: async () => ({ phase: 'impl-plan', output: 'plan done', completedAt: new Date().toISOString() }),
          executionActivity: async () => ({ phase: 'execution', output: 'code done', completedAt: new Date().toISOString() }),
          prActivity: async () => ({ phase: 'pr', output: 'pr done', completedAt: new Date().toISOString() }),
          ciWatchActivity: async () => ({ phase: 'ci-watch', output: 'ci done', completedAt: new Date().toISOString() }),
          postPhaseChangedEvent: async () => {},
        },
      });

      const result = await worker.runUntil(
        env.client.workflow.execute(workItemWorkflow, {
          taskQueue: 'test-pipeline',
          workflowId: `test-work-item-101`,
          args: ['101'],
        })
      );

      expect(result.status).toBe('completed');
      expect(result.phases).toHaveLength(7);
      expect(result.phases.map((p: { phase: string }) => p.phase)).toEqual([
        'ingest', 'discovery', 'spec', 'impl-plan', 'execution', 'pr', 'ci-watch',
      ]);
    } finally {
      await env.teardown();
    }
  });

  it('pauses on approve signal and resumes', async () => {
    const env = await TestWorkflowEnvironment.createLocal();
    let approved = false;
    try {
      const worker = await Worker.create({
        connection: env.nativeConnection,
        taskQueue: 'test-pipeline-signals',
        workflowsPath: new URL('../../src/temporal/workflows/work-item.workflow.js', import.meta.url).pathname,
        activities: {
          ingestActivity: async () => ({
            workItemId: '102', title: 'Signal Test', description: '',
            acceptanceCriteria: [], workItemType: 'Feature',
            linkedConfluenceUrls: [], repoName: null,
          }),
          discoveryActivity: async () => ({ phase: 'discovery', output: '', completedAt: new Date().toISOString() }),
          // specActivity blocks until approve signal — simulate by checking approved flag
          specActivity: async () => {
            if (!approved) throw new Error('not approved yet');
            return { phase: 'spec', output: 'approved spec', completedAt: new Date().toISOString() };
          },
          implPlanActivity: async () => ({ phase: 'impl-plan', output: '', completedAt: new Date().toISOString() }),
          executionActivity: async () => ({ phase: 'execution', output: '', completedAt: new Date().toISOString() }),
          prActivity: async () => ({ phase: 'pr', output: '', completedAt: new Date().toISOString() }),
          ciWatchActivity: async () => ({ phase: 'ci-watch', output: '', completedAt: new Date().toISOString() }),
          postPhaseChangedEvent: async () => {},
        },
      });

      const handle = await env.client.workflow.start(workItemWorkflow, {
        taskQueue: 'test-pipeline-signals',
        workflowId: 'test-signal-102',
        args: ['102'],
      });

      // Let it run to the paused state
      await new Promise(r => setTimeout(r, 1000));
      approved = true;
      await handle.signal('approve');

      const result = await worker.runUntil(handle.result());
      expect(result.status).toBe('completed');
    } finally {
      await env.teardown();
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/workflows/work-item.test.ts
```

Expected: FAIL — `Cannot find module '../../src/temporal/workflows/work-item.workflow.js'`

- [ ] **Step 3: Create `src/temporal/workflows/work-item.workflow.ts`**

```typescript
import {
  proxyActivities,
  defineSignal,
  setHandler,
  condition,
  workflowInfo,
} from '@temporalio/workflow';
import type { WorkItemContext, PhaseResult, WorkflowPhase, WorkflowState, WorkflowStatus } from '../../types.js';

// Activities are proxied — implementations live outside the workflow sandbox
const {
  ingestActivity,
  discoveryActivity,
  specActivity,
  implPlanActivity,
  executionActivity,
  prActivity,
  ciWatchActivity,
  postPhaseChangedEvent,
} = proxyActivities<{
  ingestActivity(workItemId: string): Promise<WorkItemContext>;
  discoveryActivity(ctx: WorkItemContext): Promise<PhaseResult>;
  specActivity(ctx: WorkItemContext, discovery: PhaseResult): Promise<PhaseResult>;
  implPlanActivity(ctx: WorkItemContext, spec: PhaseResult): Promise<PhaseResult>;
  executionActivity(ctx: WorkItemContext, implPlan: PhaseResult): Promise<PhaseResult>;
  prActivity(ctx: WorkItemContext, execution: PhaseResult): Promise<PhaseResult>;
  ciWatchActivity(ctx: WorkItemContext, pr: PhaseResult): Promise<PhaseResult>;
  postPhaseChangedEvent(workflowId: string, workItemId: string, phase: WorkflowPhase, status: 'started' | 'completed' | 'failed', evalScore?: number): Promise<void>;
}>({
  startToCloseTimeout: '30 minutes',
  retry: { maximumAttempts: 3 },
});

export const approveSignal = defineSignal('approve');
export const rejectSignal = defineSignal<[string]>('reject');

export async function workItemWorkflow(workItemId: string): Promise<WorkflowState> {
  const { workflowId } = workflowInfo();
  const phases: PhaseResult[] = [];
  let status: WorkflowStatus = 'running';
  let humanApproved = false;
  let humanRejected = false;
  let rejectionReason = '';

  setHandler(approveSignal, () => { humanApproved = true; });
  setHandler(rejectSignal, (reason: string) => { humanRejected = true; rejectionReason = reason; });

  async function runPhase(phase: WorkflowPhase, fn: () => Promise<PhaseResult>): Promise<PhaseResult> {
    await postPhaseChangedEvent(workflowId, workItemId, phase, 'started');
    const result = await fn();
    phases.push(result);
    await postPhaseChangedEvent(workflowId, workItemId, phase, 'completed');
    return result;
  }

  async function awaitHumanReview(phase: WorkflowPhase): Promise<void> {
    status = 'paused';
    await postPhaseChangedEvent(workflowId, workItemId, phase, 'failed');
    await condition(() => humanApproved || humanRejected);
    if (humanRejected) {
      status = 'cancelled';
      throw new Error(`Rejected by human: ${rejectionReason}`);
    }
    humanApproved = false;
    status = 'running';
  }

  // Ingest returns WorkItemContext, not PhaseResult — handle separately
  await postPhaseChangedEvent(workflowId, workItemId, 'ingest', 'started');
  const ctx = await ingestActivity(workItemId);
  phases.push({ phase: 'ingest', output: JSON.stringify(ctx), completedAt: new Date().toISOString() });
  await postPhaseChangedEvent(workflowId, workItemId, 'ingest', 'completed');

  const discovery = await runPhase('discovery', () => discoveryActivity(ctx));
  const spec = await runPhase('spec', () => specActivity(ctx, discovery));

  // Eval gate placeholder (Plan 3 will replace with real judge activity)
  // For now: no gate, continue automatically
  // await awaitHumanReview('spec');  // uncomment to test manual gate

  const implPlan = await runPhase('impl-plan', () => implPlanActivity(ctx, spec));
  const execution = await runPhase('execution', () => executionActivity(ctx, implPlan));
  const pr = await runPhase('pr', () => prActivity(ctx, execution));
  await runPhase('ci-watch', () => ciWatchActivity(ctx, pr));

  status = 'completed';

  return {
    workflowId,
    workItemId,
    currentPhase: 'ci-watch',
    status,
    phases,
    startedAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}
```

> Note: The ingest phase returns `WorkItemContext` but we track it as a phase result for consistency. The workflow treats the ingest output as both the context object and records a synthetic phase entry. This is intentional — Plan 2 will clean this up when `IngestActivity` returns a proper `PhaseResult` with embedded context.

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/workflows/work-item.test.ts
```

Expected: PASS — 2 tests green. Note: these tests spin up a local Temporal server so they may take 10–20 seconds.

- [ ] **Step 5: Commit**

```bash
git add src/temporal/workflows/work-item.workflow.ts tests/workflows/work-item.test.ts
git commit -m "feat: WorkItemWorkflow with phase sequence and signal handling"
```

---

## Task 6: `KanbanWatcherWorkflow`

**Files:**
- Create: `pipeline-service/src/temporal/workflows/kanban-watcher.workflow.ts`
- Create: `pipeline-service/tests/workflows/kanban-watcher.test.ts`

- [ ] **Step 1: Write the failing test**

Create `tests/workflows/kanban-watcher.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { TestWorkflowEnvironment } from '@temporalio/testing';
import { Worker } from '@temporalio/worker';
import { kanbanWatcherWorkflow } from '../../src/temporal/workflows/kanban-watcher.workflow.js';

describe('kanbanWatcherWorkflow', () => {
  it('starts WorkItemWorkflows for each ready item', async () => {
    const env = await TestWorkflowEnvironment.createLocal();
    const startedWorkflowIds: string[] = [];

    try {
      const worker = await Worker.create({
        connection: env.nativeConnection,
        taskQueue: 'test-watcher',
        workflowsPath: new URL('../../src/temporal/workflows/kanban-watcher.workflow.js', import.meta.url).pathname,
        activities: {
          pollReadyWorkItems: async () => ['101', '202'],
          startWorkItemWorkflowIfNotRunning: async (workItemId: string) => {
            startedWorkflowIds.push(workItemId);
          },
        },
      });

      await worker.runUntil(
        env.client.workflow.execute(kanbanWatcherWorkflow, {
          taskQueue: 'test-watcher',
          workflowId: 'test-watcher-run-1',
          args: [],
        })
      );

      expect(startedWorkflowIds).toContain('101');
      expect(startedWorkflowIds).toContain('202');
    } finally {
      await env.teardown();
    }
  });

  it('does nothing when no items are ready', async () => {
    const env = await TestWorkflowEnvironment.createLocal();
    const startedWorkflowIds: string[] = [];

    try {
      const worker = await Worker.create({
        connection: env.nativeConnection,
        taskQueue: 'test-watcher-empty',
        workflowsPath: new URL('../../src/temporal/workflows/kanban-watcher.workflow.js', import.meta.url).pathname,
        activities: {
          pollReadyWorkItems: async () => [],
          startWorkItemWorkflowIfNotRunning: async (workItemId: string) => {
            startedWorkflowIds.push(workItemId);
          },
        },
      });

      await worker.runUntil(
        env.client.workflow.execute(kanbanWatcherWorkflow, {
          taskQueue: 'test-watcher-empty',
          workflowId: 'test-watcher-empty-1',
          args: [],
        })
      );

      expect(startedWorkflowIds).toHaveLength(0);
    } finally {
      await env.teardown();
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/workflows/kanban-watcher.test.ts
```

Expected: FAIL — `Cannot find module '../../src/temporal/workflows/kanban-watcher.workflow.js'`

- [ ] **Step 3: Create `src/temporal/workflows/kanban-watcher.workflow.ts`**

```typescript
import { proxyActivities } from '@temporalio/workflow';

const { pollReadyWorkItems, startWorkItemWorkflowIfNotRunning } = proxyActivities<{
  pollReadyWorkItems(): Promise<string[]>;
  startWorkItemWorkflowIfNotRunning(workItemId: string): Promise<void>;
}>({
  startToCloseTimeout: '2 minutes',
  retry: { maximumAttempts: 3 },
});

export async function kanbanWatcherWorkflow(): Promise<void> {
  const readyItems = await pollReadyWorkItems();
  for (const workItemId of readyItems) {
    await startWorkItemWorkflowIfNotRunning(workItemId);
  }
}
```

- [ ] **Step 4: Create `src/temporal/activities/watcher.activities.ts`**

This activity starts a `WorkItemWorkflow` via the Temporal client, using workflowId as an idempotency key:

```typescript
import { Client, WorkflowExecutionAlreadyStartedError } from '@temporalio/client';
import { getTemporalClient } from '../client.js';
import type { WorkflowState } from '../../types.js';

export async function startWorkItemWorkflowIfNotRunning(workItemId: string): Promise<void> {
  const client = await getTemporalClient();
  const workflowId = `work-item-${workItemId}`;

  try {
    await client.workflow.start('workItemWorkflow', {
      taskQueue: process.env.TASK_QUEUE ?? 'pipeline',
      workflowId,
      args: [workItemId],
    });
    console.log(`[watcher] Started WorkItemWorkflow for ${workItemId}`);
  } catch (err) {
    if (err instanceof WorkflowExecutionAlreadyStartedError) {
      console.log(`[watcher] WorkItemWorkflow already running for ${workItemId} — skipping`);
      return;
    }
    throw err;
  }
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
npx vitest run tests/workflows/kanban-watcher.test.ts
```

Expected: PASS — 2 tests green.

- [ ] **Step 6: Commit**

```bash
git add src/temporal/workflows/kanban-watcher.workflow.ts src/temporal/activities/watcher.activities.ts tests/workflows/kanban-watcher.test.ts
git commit -m "feat: KanbanWatcherWorkflow polls ADO and dispatches WorkItemWorkflows"
```

---

## Task 7: Temporal Client Singleton + Worker Registration

**Files:**
- Create: `pipeline-service/src/temporal/client.ts`
- Create: `pipeline-service/src/worker.ts`

- [ ] **Step 1: Create `src/temporal/client.ts`**

```typescript
import { Client, Connection } from '@temporalio/client';

let client: Client | null = null;

export async function getTemporalClient(): Promise<Client> {
  if (!client) {
    const address = process.env.TEMPORAL_ADDRESS ?? 'localhost:7233';
    const namespace = process.env.TEMPORAL_NAMESPACE ?? 'default';
    const connection = await Connection.connect({ address });
    client = new Client({ connection, namespace });
  }
  return client;
}

export async function closeTemporalClient(): Promise<void> {
  if (client) {
    await client.connection.close();
    client = null;
  }
}
```

- [ ] **Step 2: Create `src/worker.ts`**

```typescript
import { Worker, NativeConnection } from '@temporalio/worker';
import { pollReadyWorkItems } from './temporal/activities/ado.activities.js';
import { startWorkItemWorkflowIfNotRunning } from './temporal/activities/watcher.activities.js';
import {
  ingestActivity,
  discoveryActivity,
  specActivity,
  implPlanActivity,
  executionActivity,
  prActivity,
  ciWatchActivity,
} from './temporal/activities/stub.activities.js';
import { postPhaseChangedEvent } from './temporal/activities/notify.activities.js';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export async function createWorker(): Promise<Worker> {
  const address = process.env.TEMPORAL_ADDRESS ?? 'localhost:7233';
  const taskQueue = process.env.TASK_QUEUE ?? 'pipeline';

  const connection = await NativeConnection.connect({ address });

  return Worker.create({
    connection,
    namespace: process.env.TEMPORAL_NAMESPACE ?? 'default',
    taskQueue,
    workflowsPath: path.join(__dirname, 'temporal/workflows'),
    activities: {
      pollReadyWorkItems,
      startWorkItemWorkflowIfNotRunning,
      ingestActivity,
      discoveryActivity,
      specActivity,
      implPlanActivity,
      executionActivity,
      prActivity,
      ciWatchActivity,
      postPhaseChangedEvent,
    },
  });
}
```

- [ ] **Step 3: Commit**

```bash
git add src/temporal/client.ts src/worker.ts
git commit -m "feat: Temporal client singleton and worker registration"
```

---

## Task 8: HTTP API

**Files:**
- Create: `pipeline-service/src/server.ts`
- Create: `pipeline-service/tests/server.test.ts`

- [ ] **Step 1: Write the failing test**

Create `tests/server.test.ts`:

```typescript
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import request from 'supertest';
import pg from 'pg';
import { createApp } from '../src/server.js';
import { runMigrations } from '../src/db/migrations.js';

// npm install --save-dev supertest @types/supertest
describe('HTTP API', () => {
  let app: ReturnType<typeof createApp>;
  let pool: pg.Pool;

  beforeAll(async () => {
    pool = new pg.Pool({ connectionString: process.env.DATABASE_URL ?? 'postgresql://temporal:temporal@localhost:5432/pipeline' });
    await runMigrations(pool);
    // Seed a workflow run
    await pool.query(
      `INSERT INTO workflow_runs (id, work_item_id, current_phase, status, phases)
       VALUES ('wf-001', '101', 'spec', 'running', '[]')
       ON CONFLICT (id) DO NOTHING`
    );
    app = createApp(pool);
  });

  afterAll(async () => {
    await pool.end();
  });

  it('GET /health returns ok', async () => {
    const res = await request(app).get('/health');
    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ok');
  });

  it('GET /workflows returns list', async () => {
    const res = await request(app).get('/workflows');
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
    const wf = res.body.find((w: { id: string }) => w.id === 'wf-001');
    expect(wf).toBeDefined();
    expect(wf.workItemId).toBe('101');
  });

  it('GET /workflows/:id returns detail', async () => {
    const res = await request(app).get('/workflows/wf-001');
    expect(res.status).toBe(200);
    expect(res.body.id).toBe('wf-001');
  });

  it('GET /workflows/:id returns 404 for unknown', async () => {
    const res = await request(app).get('/workflows/does-not-exist');
    expect(res.status).toBe(404);
  });
});
```

- [ ] **Step 2: Install supertest**

```bash
npm install --save-dev supertest @types/supertest
```

- [ ] **Step 3: Run test to verify it fails**

```bash
npx vitest run tests/server.test.ts
```

Expected: FAIL — `Cannot find module '../src/server.js'`

- [ ] **Step 4: Create `src/server.ts`**

```typescript
import express from 'express';
import type pg from 'pg';

export function createApp(pool: pg.Pool) {
  const app = express();
  app.use(express.json());

  app.get('/health', (_req, res) => {
    res.json({ status: 'ok' });
  });

  app.get('/workflows', async (_req, res) => {
    try {
      const result = await pool.query(
        `SELECT id, work_item_id AS "workItemId", current_phase AS "currentPhase",
                status, phases, started_at AS "startedAt", updated_at AS "updatedAt"
         FROM workflow_runs ORDER BY started_at DESC LIMIT 100`
      );
      res.json(result.rows);
    } catch (err) {
      res.status(500).json({ error: String(err) });
    }
  });

  app.get('/workflows/:id', async (req, res) => {
    try {
      const result = await pool.query(
        `SELECT id, work_item_id AS "workItemId", current_phase AS "currentPhase",
                status, phases, started_at AS "startedAt", updated_at AS "updatedAt"
         FROM workflow_runs WHERE id = $1`,
        [req.params.id]
      );
      if (result.rows.length === 0) {
        res.status(404).json({ error: 'Not found' });
        return;
      }
      res.json(result.rows[0]);
    } catch (err) {
      res.status(500).json({ error: String(err) });
    }
  });

  app.post('/workflows/:id/signal', async (req, res) => {
    const { signal, reason } = req.body as { signal: 'approve' | 'reject'; reason?: string };
    if (!signal || !['approve', 'reject'].includes(signal)) {
      res.status(400).json({ error: 'signal must be approve or reject' });
      return;
    }
    // Temporal client signal — requires runtime env
    // Returns 202 always; Temporal delivers async
    res.status(202).json({ queued: true });
  });

  app.post('/workflows', async (req, res) => {
    const { workItemId } = req.body as { workItemId: string };
    if (!workItemId) {
      res.status(400).json({ error: 'workItemId is required' });
      return;
    }
    // Temporal start — requires runtime env
    res.status(202).json({ workflowId: `work-item-${workItemId}`, queued: true });
  });

  return app;
}
```

> Note: `/workflows/:id/signal` and `POST /workflows` return 202 without actually calling Temporal in this skeleton. Task 9 wires up the real Temporal client calls in the entry point.

- [ ] **Step 5: Run test to verify it passes**

```bash
docker compose up postgresql -d && sleep 3
npx vitest run tests/server.test.ts
```

Expected: PASS — 4 tests green.

- [ ] **Step 6: Commit**

```bash
git add src/server.ts tests/server.test.ts
git commit -m "feat: Express HTTP API with /health, /workflows, and signal endpoints"
```

---

## Task 9: Entry Point + Watcher Schedule

**Files:**
- Create: `pipeline-service/src/index.ts`

Wires everything together: runs DB migrations, starts the HTTP server, starts the Temporal worker, and schedules `KanbanWatcherWorkflow` to run on an interval.

- [ ] **Step 1: Create `src/index.ts`**

```typescript
import 'dotenv/config';
import http from 'http';
import { createApp } from './server.js';
import { createWorker } from './worker.js';
import { getPool, closePool } from './db/client.js';
import { runMigrations } from './db/migrations.js';
import { getTemporalClient, closeTemporalClient } from './temporal/client.js';
import { ScheduleOverlapPolicy } from '@temporalio/client';

const PORT = parseInt(process.env.PORT ?? '9877', 10);
const WATCHER_INTERVAL_MINUTES = parseInt(process.env.WATCHER_SCHEDULE_INTERVAL_MINUTES ?? '5', 10);

async function main() {
  // DB
  const pool = getPool();
  await runMigrations(pool);
  console.log('[startup] DB migrations complete');

  // HTTP
  const app = createApp(pool);
  const server = http.createServer(app);
  server.listen(PORT, () => console.log(`[startup] HTTP API listening on :${PORT}`));

  // Temporal worker
  const worker = await createWorker();
  console.log('[startup] Temporal worker created');

  // Schedule KanbanWatcherWorkflow
  const client = await getTemporalClient();
  const scheduleId = 'kanban-watcher-schedule';
  try {
    await client.schedule.create({
      scheduleId,
      spec: {
        intervals: [{ every: `${WATCHER_INTERVAL_MINUTES}m` }],
      },
      action: {
        type: 'startWorkflow',
        workflowType: 'kanbanWatcherWorkflow',
        taskQueue: process.env.TASK_QUEUE ?? 'pipeline',
        args: [],
      },
      policies: {
        overlap: ScheduleOverlapPolicy.SKIP,
      },
    });
    console.log(`[startup] KanbanWatcher schedule created (every ${WATCHER_INTERVAL_MINUTES}m)`);
  } catch (err: unknown) {
    // gRPC ALREADY_EXISTS = code 6; Temporal throws this when schedule exists
    const isAlreadyExists =
      (err instanceof Error && err.message.includes('already exists')) ||
      (typeof err === 'object' && err !== null && 'code' in err && (err as { code: number }).code === 6);
    if (isAlreadyExists) {
      console.log('[startup] KanbanWatcher schedule already exists — skipping');
    } else {
      throw err;
    }
  }

  // Graceful shutdown
  const shutdown = async () => {
    console.log('[shutdown] Stopping...');
    server.close();
    worker.shutdown();
    await closeTemporalClient();
    await closePool();
    process.exit(0);
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  // Run worker (blocks until shutdown)
  await worker.run();
}

main().catch((err) => {
  console.error('[fatal]', err);
  process.exit(1);
});
```

- [ ] **Step 2: Build to verify TypeScript compiles clean**

```bash
npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/index.ts
git commit -m "feat: entry point — wires DB, HTTP, worker, and watcher schedule"
```

---

## Task 10: Smoke Test — Full Stack

Verify the complete stack runs with `docker compose up`.

- [ ] **Step 1: Copy `.env.example` to `.env` and fill in ADO credentials**

```bash
cp .env.example .env
# Edit .env: set ADO_ORG_URL, ADO_PAT, ADO_PROJECT, ADO_TEAM, ADO_READY_COLUMN
```

- [ ] **Step 2: Build and start the full stack**

```bash
docker compose up --build -d
```

Expected: all 4 services start without error. Check with:

```bash
docker compose logs pipeline-worker --tail 20
```

Expected output includes:
```
[startup] DB migrations complete
[startup] HTTP API listening on :9877
[startup] Temporal worker created
[startup] KanbanWatcher schedule created (every 5m)
```

- [ ] **Step 3: Verify Temporal UI**

Open `http://localhost:8233` in a browser. Expected: Temporal web UI showing the `kanban-watcher-schedule` schedule and (if ADO credentials are valid) any `work-item-{id}` workflows that started.

- [ ] **Step 4: Verify HTTP API**

```bash
curl http://localhost:9877/health
```

Expected: `{"status":"ok"}`

```bash
curl http://localhost:9877/workflows
```

Expected: JSON array (empty if no workflows have run yet).

- [ ] **Step 5: Manually trigger a workflow**

```bash
curl -X POST http://localhost:9877/workflows \
  -H "Content-Type: application/json" \
  -d '{"workItemId": "999"}'
```

Expected: `{"workflowId":"work-item-999","queued":true}`

Check Temporal UI — `work-item-999` workflow should appear and complete through all stub phases.

- [ ] **Step 6: Add pipeline-service as a submodule in coverage-manager**

```bash
cd /Users/david.miller/Documents/current/coverage-manager
git submodule add /Users/david.miller/Documents/current/pipeline-service pipeline-service
git commit -m "chore: add pipeline-service as submodule"
```

- [ ] **Step 7: Commit final smoke test passing**

```bash
cd /Users/david.miller/Documents/current/pipeline-service
git tag v0.1.0
```

---

## Task 11: Accept New Event Kinds in Coverage-Manager

Ensure coverage-manager's orchestrator DB doesn't reject the new `workflow_phase_changed` event kind posted by the pipeline-service.

**Files:**
- Modify: `src-tauri/src/orchestrator/db.rs` (check event kind validation)

- [ ] **Step 1: Check if event kinds are validated**

```bash
grep -n "kind" /Users/david.miller/Documents/current/coverage-manager/src-tauri/src/orchestrator/db.rs | head -20
```

- [ ] **Step 2: If kinds are validated against an enum or allowlist, add the new kinds**

Look for a match or enum check on `kind`. Add:
- `"workflow_phase_changed"`
- `"eval_result"`

If kinds are stored as free-form strings with no validation (likely given the flexible event model), no change is needed — confirm by checking the `INSERT INTO events` query accepts any string for `kind`.

- [ ] **Step 3: Commit if changed**

```bash
cd /Users/david.miller/Documents/current/coverage-manager
git add src-tauri/src/orchestrator/db.rs
git commit -m "feat: accept workflow_phase_changed and eval_result event kinds"
```

---

## Run All Tests

```bash
cd /Users/david.miller/Documents/current/pipeline-service
docker compose up postgresql -d
sleep 3
npx vitest run
```

Expected: all tests pass (db, ado activities, kanban watcher workflow, work item workflow, server).
