export type BaseTheme = 'dark' | 'light'

export const darkTokens: Record<string, string> = {
  '--bg-base':     '#0f0f13',
  '--bg-surface':  '#1a1a1e',
  '--bg-panel':    '#13131a',
  '--bg-hover':    '#22222a',
  '--border':      '#2a2a2e',
  '--border-subtle': '#1e1e24',
  '--text-primary':   '#e8e8ec',
  '--text-secondary': '#a0a0b0',
  '--text-muted':     '#555566',
  '--accent':      '#4f8ef7',
  '--accent-muted': '#4f8ef722',
  '--destructive': '#c0504a',
  '--success':     '#4a9a5a',
}

export const lightTokens: Record<string, string> = {
  '--bg-base':     '#f4f4f8',
  '--bg-surface':  '#ffffff',
  '--bg-panel':    '#eaeaf0',
  '--bg-hover':    '#e0e0ea',
  '--border':      '#d0d0da',
  '--border-subtle': '#e4e4ec',
  '--text-primary':   '#0f0f18',
  '--text-secondary': '#50506a',
  '--text-muted':     '#9090a8',
  '--accent':      '#2a6ae0',
  '--accent-muted': '#2a6ae020',
  '--destructive': '#c0504a',
  '--success':     '#3a8a4a',
}

export function applyTokens(
  tokens: Record<string, string>,
  accentColor?: string | null,
): void {
  const root = document.documentElement
  for (const [k, v] of Object.entries(tokens)) {
    root.style.setProperty(k, v)
  }
  if (accentColor) {
    root.style.setProperty('--accent', accentColor)
    root.style.setProperty('--accent-muted', accentColor + '22')
  }
}
