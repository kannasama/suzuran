import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { darkTokens, lightTokens, applyTokens, type BaseTheme } from './tokens'

interface ThemeContextValue {
  baseTheme: BaseTheme
  accentColor: string | null
  backgroundUrl: string | null
  setTheme: (base: BaseTheme, accent?: string | null, bgUrl?: string | null) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  baseTheme: 'dark',
  accentColor: null,
  backgroundUrl: null,
  setTheme: () => {},
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

  useEffect(() => {
    const tokens = baseTheme === 'dark' ? darkTokens : lightTokens
    applyTokens(tokens, accentColor)

    // Toggle dark class on <html> for Tailwind
    document.documentElement.classList.toggle('dark', baseTheme === 'dark')
  }, [baseTheme, accentColor])

  useEffect(() => {
    const root = document.documentElement
    if (backgroundUrl) {
      root.style.setProperty('--theme-bg-image', `url('${backgroundUrl}')`)
      root.classList.add('has-theme-bg')
    } else {
      root.style.removeProperty('--theme-bg-image')
      root.classList.remove('has-theme-bg')
    }
  }, [backgroundUrl])

  const setTheme = (base: BaseTheme, accent?: string | null, bgUrl?: string | null) => {
    setBaseTheme(base)
    if (accent !== undefined) setAccentColor(accent)
    if (bgUrl !== undefined) setBackgroundUrl(bgUrl)
  }

  return (
    <ThemeContext.Provider value={{ baseTheme, accentColor, backgroundUrl, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}
