// Theme settings: hex color values persisted to localStorage and applied
// to CSS custom properties on :root. Mirrors the pattern used by the
// meeting-tool sibling project.

export const THEME_DEFAULTS = {
  bg: '#ffffff',
  bgSubtle: '#f6f7f9',
  bgMuted: '#edf0f3',
  border: '#dde1e7',
  borderSubtle: '#eeeff2',
  text: '#111827',
  textSecondary: '#4b5563',
  textMuted: '#9ca3af',
  accent: '#2563eb',
  accentHover: '#1d4ed8',
  accentSubtle: '#eff6ff',
  success: '#16a34a',
  warning: '#d97706',
  danger: '#dc2626',
} as const;

export type ThemeSettings = { -readonly [K in keyof typeof THEME_DEFAULTS]: string };
export type ThemeKey = keyof ThemeSettings;

const CSS_VARIABLES: Record<ThemeKey, string> = {
  bg: '--bg',
  bgSubtle: '--bg-subtle',
  bgMuted: '--bg-muted',
  border: '--border',
  borderSubtle: '--border-subtle',
  text: '--text',
  textSecondary: '--text-secondary',
  textMuted: '--text-muted',
  accent: '--accent',
  accentHover: '--accent-hover',
  accentSubtle: '--accent-subtle',
  success: '--success',
  warning: '--warning',
  danger: '--danger',
};

export const THEME_FIELDS: Array<{ key: ThemeKey; label: string; hint?: string }> = [
  { key: 'bg', label: 'Background' },
  { key: 'bgSubtle', label: 'Background (subtle)' },
  { key: 'bgMuted', label: 'Background (muted)' },
  { key: 'border', label: 'Border' },
  { key: 'borderSubtle', label: 'Border (subtle)' },
  { key: 'text', label: 'Text' },
  { key: 'textSecondary', label: 'Text (secondary)' },
  { key: 'textMuted', label: 'Text (muted)' },
  { key: 'accent', label: 'Accent' },
  { key: 'accentHover', label: 'Accent (hover)' },
  { key: 'accentSubtle', label: 'Accent (subtle)' },
  { key: 'success', label: 'Success' },
  { key: 'warning', label: 'Warning' },
  { key: 'danger', label: 'Danger' },
];

const STORAGE_KEY = 'coverage-manager-theme';

function normalizeHex(value: string | null | undefined, fallback: string): string {
  if (!value) return fallback;
  const v = value.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(v)) return v;
  if (/^#[0-9a-fA-F]{3}$/.test(v)) {
    // Expand short form #abc -> #aabbcc
    return '#' + v.slice(1).split('').map((c) => c + c).join('');
  }
  return fallback;
}

export function loadThemeSettings(): ThemeSettings {
  const theme: ThemeSettings = { ...(THEME_DEFAULTS as ThemeSettings) };
  if (typeof localStorage === 'undefined') return theme;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return theme;
    const parsed = JSON.parse(raw) as Partial<Record<string, string>>;
    for (const key of Object.keys(THEME_DEFAULTS) as ThemeKey[]) {
      theme[key] = normalizeHex(parsed[key], THEME_DEFAULTS[key]);
    }
  } catch {
    // Ignore malformed local state and use defaults.
  }
  return theme;
}

export function applyThemeSettings(theme: ThemeSettings): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  for (const key of Object.keys(theme) as ThemeKey[]) {
    root.style.setProperty(CSS_VARIABLES[key], theme[key]);
  }
}

export function saveThemeSettings(theme: ThemeSettings): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(theme));
}

export function setThemeColor(
  theme: ThemeSettings,
  key: ThemeKey,
  value: string,
): { theme: ThemeSettings; normalized: string } {
  const normalized = normalizeHex(value, THEME_DEFAULTS[key]);
  const next = { ...theme, [key]: normalized };
  if (typeof document !== 'undefined') {
    document.documentElement.style.setProperty(CSS_VARIABLES[key], normalized);
  }
  saveThemeSettings(next);
  return { theme: next, normalized };
}

export function resetThemeSettings(): ThemeSettings {
  const theme: ThemeSettings = { ...(THEME_DEFAULTS as ThemeSettings) };
  applyThemeSettings(theme);
  saveThemeSettings(theme);
  return theme;
}
