import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useQuery } from '@tanstack/react-query'
import { darkTokens, lightTokens, applyTokens, type BaseTheme } from './tokens'
import { listThemes, type Theme } from '../api/themes'

interface ThemeContextValue {
  baseTheme: BaseTheme
  accentColor: string | null
  backgroundUrl: string | null
  setTheme: (base: BaseTheme, accent?: string | null, bgUrl?: string | null) => void
  themes: Theme[]
  activeThemeId: number | null
  setActiveTheme: (id: number | null) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  baseTheme: 'dark',
  accentColor: null,
  backgroundUrl: null,
  setTheme: () => {},
  themes: [],
  activeThemeId: null,
  setActiveTheme: () => {},
})

export function useTheme() {
  return useContext(ThemeContext)
}

interface ThemeProviderProps {
  children: ReactNode
  initialBase?: BaseTheme
  initialAccent?: string | null
  initialBackgroundUrl?: string | null
}

export function ThemeProvider({
  children,
  initialBase = 'dark',
  initialAccent = null,
  initialBackgroundUrl = null,
}: ThemeProviderProps) {
  const [baseTheme, setBaseTheme] = useState<BaseTheme>(initialBase)
  const [accentColor, setAccentColor] = useState<string | null>(initialAccent)
  const [backgroundUrl, setBackgroundUrl] = useState<string | null>(initialBackgroundUrl)

  const [activeThemeId, setActiveThemeIdState] = useState<number | null>(() => {
    const stored = localStorage.getItem('suzuran:active-theme')
    return stored ? Number(stored) : null
  })

  // Load themes from DB — retry: false so 401 on login page doesn't spam
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
    const extraVars = activeTheme ? activeTheme.css_vars : null
    applyTokens(tokens, effectiveAccent, extraVars)
    document.documentElement.classList.toggle('dark', baseTheme === 'dark')
  }, [baseTheme, accentColor, activeTheme])

  // Apply background image from active theme or manual override
  useEffect(() => {
    const effectiveBgUrl = activeTheme?.background_url ?? backgroundUrl
    const root = document.documentElement
    if (effectiveBgUrl) {
      root.style.setProperty('--theme-bg-image', `url('${effectiveBgUrl}')`)
      root.classList.add('has-theme-bg')
    } else {
      root.style.removeProperty('--theme-bg-image')
      root.classList.remove('has-theme-bg')
    }
  }, [backgroundUrl, activeTheme])

  function setActiveTheme(id: number | null) {
    setActiveThemeIdState(id)
    if (id === null) {
      localStorage.removeItem('suzuran:active-theme')
    } else {
      localStorage.setItem('suzuran:active-theme', String(id))
    }
  }

  const setTheme = (base: BaseTheme, accent?: string | null, bgUrl?: string | null) => {
    setBaseTheme(base)
    if (accent !== undefined) setAccentColor(accent)
    if (bgUrl !== undefined) setBackgroundUrl(bgUrl)
  }

  return (
    <ThemeContext.Provider value={{
      baseTheme,
      accentColor,
      backgroundUrl,
      setTheme,
      themes,
      activeThemeId,
      setActiveTheme,
    }}>
      {children}
    </ThemeContext.Provider>
  )
}
