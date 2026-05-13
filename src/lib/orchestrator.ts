const BASE = 'http://127.0.0.1:9876';

export type SessionStatus = 'working' | 'idle' | 'needs_input' | 'done' | 'error' | 'unknown';

export type ArtifactKind = 'project' | 'ado' | 'confluence' | 'github_pr';

export interface Session {
  id: string;
  label: string | null;
  cwd: string;
  pid: number;
  status: SessionStatus;
  current_tool: string | null;
  last_progress: string | null;
  last_user_prompt: string | null;
  started_at: number;
  updated_at: number;
  ended_at: number | null;
  end_reason: string | null;
  transcript_path: string | null;
  artifact_kind: ArtifactKind | null;
  artifact_id: string | null;
  artifact_title: string | null;
  artifact_url: string | null;
  loaded_snapshot: string | null;
}

export interface LinkBody {
  kind: ArtifactKind;
  id: string;
  title?: string | null;
  url?: string | null;
}

export interface TranscriptTurn {
  role: 'user' | 'assistant';
  text: string;
  ts: number;
}

export interface OrchestratorEvent {
  id: number;
  ts: number;
  kind: string;
  payload: string;
}

export interface Artifact {
  id: number;
  session_id: string;
  ts: number;
  path: string;
  label: string | null;
  kind: string;
}

export async function listSessions(): Promise<Session[]> {
  const r = await fetch(`${BASE}/sessions`);
  if (!r.ok) throw new Error(`listSessions ${r.status}`);
  return (await r.json()).sessions ?? [];
}

export async function listEvents(sid: string, limit = 200): Promise<OrchestratorEvent[]> {
  const r = await fetch(`${BASE}/events?session=${encodeURIComponent(sid)}&limit=${limit}`);
  return r.ok ? (await r.json()).events ?? [] : [];
}

export async function listArtifacts(sid: string): Promise<Artifact[]> {
  const r = await fetch(`${BASE}/artifacts?session=${encodeURIComponent(sid)}`);
  return r.ok ? (await r.json()).artifacts ?? [] : [];
}

export async function sendInbox(sid: string, message: string): Promise<void> {
  await fetch(`${BASE}/inbox/${encodeURIComponent(sid)}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ from_kind: 'human', message })
  });
}

export async function linkArtifact(sid: string, body: LinkBody): Promise<void> {
  const r = await fetch(`${BASE}/sessions/${encodeURIComponent(sid)}/link`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body)
  });
  if (!r.ok) throw new Error(`linkArtifact ${r.status}`);
}

export async function unlinkArtifact(sid: string): Promise<void> {
  const r = await fetch(`${BASE}/sessions/${encodeURIComponent(sid)}/link`, {
    method: 'DELETE'
  });
  if (!r.ok) throw new Error(`unlinkArtifact ${r.status}`);
}

export async function getTranscriptTail(sid: string, turns = 2): Promise<TranscriptTurn[]> {
  const r = await fetch(`${BASE}/sessions/${encodeURIComponent(sid)}/transcript-tail?turns=${turns}`);
  if (!r.ok) return [];
  return (await r.json()).turns ?? [];
}
