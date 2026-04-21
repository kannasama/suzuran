export type BaseTheme = 'dark' | 'light'

export const ACCENT_COLORS = {
  indigo:         '#4f8ef7',
  blue:           '#3b82f6',
  cyan:           '#06b6d4',
  teal:           '#14b8a6',
  'bright-teal':  '#2dd4bf',
  lime:           '#84cc16',
  'acid-lime':    '#a3e635',
  yellow:         '#eab308',
  orange:         '#f97316',
  tomato:         '#ef4444',
  pink:           '#ec4899',
  purple:         '#a855f7',
  vira:           '#7c83d1',
  white:          '#f1f5f9',
} as const

export type AccentName = keyof typeof ACCENT_COLORS

export const darkTokens: Record<string, string> = {
  '--bg-base':        '#0f0f13',
  '--bg-surface':     '#1a1a1e',
  '--bg-panel':       '#13131a',
  '--bg-elevated':    '#1e1e26',
  '--bg-hover':       '#22222a',
  '--border':         '#2a2a2e',
  '--border-subtle':  '#1e1e24',
  '--surface-border': '#2a2a2e',
  '--text-primary':   '#e8e8ec',
  '--text-secondary': '#a0a0b0',
  '--text-muted':     '#555566',
  '--text-disabled':  '#3a3a4a',
  '--destructive':    '#c0504a',
  '--success':        '#4a9a5a',
}

export const lightTokens: Record<string, string> = {
  '--bg-base':        '#f4f4f8',
  '--bg-surface':     '#ffffff',
  '--bg-panel':       '#eaeaf0',
  '--bg-elevated':    '#f0f0f6',
  '--bg-hover':       '#e0e0ea',
  '--border':         '#d0d0da',
  '--border-subtle':  '#e4e4ec',
  '--surface-border': '#d0d0da',
  '--text-primary':   '#0f0f18',
  '--text-secondary': '#50506a',
  '--text-muted':     '#9090a8',
  '--text-disabled':  '#c0c0ce',
  '--destructive':    '#c0504a',
  '--success':        '#3a8a4a',
}

/**
 * Apply a base token set to :root.
 *
 * For named accents: sets `data-accent` on <html> so the CSS rules in
 * index.css apply --accent, --accent-hover, --accent-muted, --accent-rgb
 * all at once. Any previous inline accent overrides are removed.
 *
 * For custom hex accents (not in ACCENT_COLORS): removes data-accent and
 * sets --accent inline; derives --accent-rgb for Tailwind opacity modifiers.
 *
 * extraVars: additional CSS vars from a custom theme (e.g. extracted palette).
 * These override base tokens so the image-derived surface colors take effect.
 */
export function applyTokens(
  tokens: Record<string, string>,
  accentColor?: string | null,
  extraVars?: Record<string, string> | null,
): void {
  const root = document.documentElement

  // Apply base tokens
  for (const [k, v] of Object.entries(tokens)) {
    root.style.setProperty(k, v)
  }

  // Apply custom theme vars on top (these override the base tokens above)
  if (extraVars) {
    for (const [k, v] of Object.entries(extraVars)) {
      root.style.setProperty(k, v)
    }
  }

  // Apply accent
  const accentNames = Object.keys(ACCENT_COLORS) as AccentName[]
  const isNamed = accentColor && accentNames.includes(accentColor as AccentName)

  if (!accentColor || isNamed) {
    // Use data-accent CSS rules — remove any inline overrides
    root.setAttribute('data-accent', (accentColor as AccentName) ?? 'indigo')
    root.style.removeProperty('--accent')
    root.style.removeProperty('--accent-rgb')
    root.style.removeProperty('--accent-hover')
    root.style.removeProperty('--accent-muted')
  } else {
    // Custom hex — set inline and derive --accent-rgb
    root.removeAttribute('data-accent')
    root.style.setProperty('--accent', accentColor)
    const hex = accentColor.replace('#', '')
    const r = parseInt(hex.slice(0, 2), 16)
    const g = parseInt(hex.slice(2, 4), 16)
    const b = parseInt(hex.slice(4, 6), 16)
    if (!isNaN(r) && !isNaN(g) && !isNaN(b)) {
      root.style.setProperty('--accent-rgb', `${r} ${g} ${b}`)
    }
    // Derive a simple hover (lighten by ~10%)
    root.style.removeProperty('--accent-hover')
    root.style.removeProperty('--accent-muted')
  }
}
