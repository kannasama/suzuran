export type BaseTheme = 'dark' | 'light'

export const ACCENT_COLORS = {
  indigo:       '#4f8ef7',
  blue:         '#3b82f6',
  cyan:         '#06b6d4',
  teal:         '#14b8a6',
  'bright-teal':'#2dd4bf',
  lime:         '#84cc16',
  'acid-lime':  '#a3e635',
  yellow:       '#eab308',
  orange:       '#f97316',
  tomato:       '#ef4444',
  pink:         '#ec4899',
  purple:       '#a855f7',
  vira:         '#7c83d1',
  white:        '#f1f5f9',
} as const

export type AccentName = keyof typeof ACCENT_COLORS

export const darkTokens: Record<string, string> = {
  '--bg-base':        '#0f0f13',
  '--bg-surface':     '#1a1a1e',
  '--bg-panel':       '#13131a',
  '--bg-hover':       '#22222a',
  '--border':         '#2a2a2e',
  '--border-subtle':  '#1e1e24',
  '--text-primary':   '#e8e8ec',
  '--text-secondary': '#a0a0b0',
  '--text-muted':     '#555566',
  '--accent':         '#4f8ef7',
  '--accent-muted':   '#4f8ef722',
  '--destructive':    '#c0504a',
  '--success':        '#4a9a5a',
}

export const lightTokens: Record<string, string> = {
  '--bg-base':        '#f4f4f8',
  '--bg-surface':     '#ffffff',
  '--bg-panel':       '#eaeaf0',
  '--bg-hover':       '#e0e0ea',
  '--border':         '#d0d0da',
  '--border-subtle':  '#e4e4ec',
  '--text-primary':   '#0f0f18',
  '--text-secondary': '#50506a',
  '--text-muted':     '#9090a8',
  '--accent':         '#2a6ae0',
  '--accent-muted':   '#2a6ae020',
  '--destructive':    '#c0504a',
  '--success':        '#3a8a4a',
}

/** Convert a #rrggbb hex string to space-separated "r g b" integers for CSS. */
export function hexToRgbChannels(hex: string): string {
  const h = hex.replace('#', '')
  const r = parseInt(h.slice(0, 2), 16)
  const g = parseInt(h.slice(2, 4), 16)
  const b = parseInt(h.slice(4, 6), 16)
  if (isNaN(r) || isNaN(g) || isNaN(b)) return '79 142 247' // fallback indigo
  return `${r} ${g} ${b}`
}

export function applyTokens(
  tokens: Record<string, string>,
  accentColor?: string | null,
  extraVars?: Record<string, string> | null,
): void {
  const root = document.documentElement
  for (const [k, v] of Object.entries(tokens)) {
    root.style.setProperty(k, v)
  }
  // Extra CSS vars from custom theme (e.g. background-tinted surface colors)
  if (extraVars) {
    for (const [k, v] of Object.entries(extraVars)) {
      root.style.setProperty(k, v)
    }
  }
  // Apply accent and derive --accent-rgb so Tailwind opacity modifiers work
  const accent = accentColor ?? tokens['--accent']
  if (accent) {
    root.style.setProperty('--accent', accent)
    root.style.setProperty('--accent-rgb', hexToRgbChannels(accent))
    root.style.setProperty('--accent-muted', accent + '22')
  }
}
