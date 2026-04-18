import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { darkTokens, lightTokens, applyTokens, type BaseTheme } from './tokens'

interface ThemeContextValue {
  baseTheme: BaseTheme
  accentColor: string | null
  setTheme: (base: BaseTheme, accent?: string | null) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  baseTheme: 'dark',
  accentColor: null,
  setTheme: () => {},
})

export function useTheme() {
  return useContext(ThemeContext)
}

interface ThemeProviderProps {
  children: ReactNode
  initialBase?: BaseTheme
  initialAccent?: string | null
}

export function ThemeProvider({
  children,
  initialBase = 'dark',
  initialAccent = null,
}: ThemeProviderProps) {
  const [baseTheme, setBaseTheme] = useState<BaseTheme>(initialBase)
  const [accentColor, setAccentColor] = useState<string | null>(initialAccent)

  useEffect(() => {
    const tokens = baseTheme === 'dark' ? darkTokens : lightTokens
    applyTokens(tokens, accentColor)

    // Toggle dark class on <html> for Tailwind
    document.documentElement.classList.toggle('dark', baseTheme === 'dark')
  }, [baseTheme, accentColor])

  const setTheme = (base: BaseTheme, accent?: string | null) => {
    setBaseTheme(base)
    if (accent !== undefined) setAccentColor(accent)
  }

  return (
    <ThemeContext.Provider value={{ baseTheme, accentColor, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}
