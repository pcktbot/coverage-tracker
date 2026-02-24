import { writable, derived } from 'svelte/store';
import type { Org, Repo } from '$lib/api';
import { listOrgs, listRepos, getActiveOrg } from '$lib/api';

export const orgs = writable<Org[]>([]);
export const activeOrg = writable<string | null>(null);
export const repos = writable<Repo[]>([]);

export const enabledRepos = derived(repos, ($repos) => $repos.filter((r) => r.enabled));

export async function refreshOrgs(): Promise<void> {
  const [all, active] = await Promise.all([listOrgs(), getActiveOrg()]);
  orgs.set(all);
  activeOrg.set(active);
}

export async function refreshRepos(org?: string): Promise<void> {
  const list = await listRepos(org);
  repos.set(list);
}
