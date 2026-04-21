import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useQuery } from '@tanstack/react-query'
import { darkTokens, lightTokens, applyTokens, type BaseTheme } from './tokens'
import { listThemes, type Theme } from '../api/themes'

interface ThemeContextValue {
  baseTheme: BaseTheme
  accentColor: string | null
  setTheme: (base: BaseTheme, accent?: string | null) => void
  themes: Theme[]
  activeThemeId: number | null
  setActiveTheme: (id: number | null) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  baseTheme: 'dark',
  accentColor: null,
  setTheme: () => {},
  themes: [],
  activeThemeId: null,
  setActiveTheme: () => {},
})

export function useTheme() {
  return useContext(ThemeContext)
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [baseTheme, setBaseTheme] = useState<BaseTheme>('dark')
  const [accentColor, setAccentColor] = useState<string | null>(null)

  const [activeThemeId, setActiveThemeIdState] = useState<number | null>(() => {
    const stored = localStorage.getItem('suzuran:active-theme')
    return stored ? Number(stored) : null
  })

  // Load themes — retry: false so 401 on login page doesn't spam
  const { data: themes = [] } = useQuery({
    queryKey: ['themes'],
    queryFn: listThemes,
    retry: false,
    staleTime: 60_000,
  })

  const activeTheme = themes.find(t => t.id === activeThemeId) ?? null

  // Apply base tokens + active theme overlay whenever either changes
  useEffect(() => {
    const tokens = baseTheme === 'dark' ? darkTokens : lightTokens
    const effectiveAccent = activeTheme?.accent_color ?? accentColor
    const extraVars = activeTheme?.css_vars
      ? (activeTheme.css_vars as Record<string, string>)
      : null
    applyTokens(tokens, effectiveAccent, extraVars)
  }, [baseTheme, accentColor, activeTheme])

  // Apply background image directly to body so semi-transparent surfaces show it through
  useEffect(() => {
    const bgUrl = activeTheme?.background_url ?? null
    if (bgUrl) {
      document.body.style.backgroundImage = `url('${bgUrl}')`
    } else {
      document.body.style.backgroundImage = ''
    }
  }, [activeTheme])

  function setActiveTheme(id: number | null) {
    setActiveThemeIdState(id)
    if (id === null) {
      localStorage.removeItem('suzuran:active-theme')
    } else {
      localStorage.setItem('suzuran:active-theme', String(id))
    }
  }

  function setTheme(base: BaseTheme, accent?: string | null) {
    setBaseTheme(base)
    if (accent !== undefined) setAccentColor(accent)
  }

  return (
    <ThemeContext.Provider value={{
      baseTheme,
      accentColor,
      setTheme,
      themes,
      activeThemeId,
      setActiveTheme,
    }}>
      {children}
    </ThemeContext.Provider>
  )
}
