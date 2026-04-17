# Phase 1.10 — UI Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bootstrap the React + Vite + Tailwind SPA — auth pages (login, register), top nav, library view skeleton, and a `ThemeProvider` that applies the user's chosen base theme and accent color. The built static files are embedded in and served by the Axum backend. The UI makes real API calls; it is not mocked.

**Architecture:** `ui/` is a standalone Vite project. `npm run build` outputs to `ui/dist/`. The Dockerfile Stage 2 (ui-builder) runs `npm ci && npm run build`. The Axum backend serves `ui/dist/` as static files via `tower-http::ServeDir` on all non-API routes (SPA fallback). Theming uses CSS variables set on `<html>` by `ThemeProvider`; dark mode is the default.

**Design reference:** `.impeccable.md` — foobar2000 DarkOne aesthetic: near-black background (`#1a1a1e`), cool blue accent (`#4f8ef7`), dense tabular layouts, minimal chrome.

**Tech Stack:** React 18, Vite 5, Tailwind CSS 3, React Router 6, TanStack Query 5, Axios (or fetch).

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `ui/package.json` | Create | Vite + React + Tailwind + Router + Query deps |
| `ui/vite.config.ts` | Create | Vite config with API proxy for dev |
| `ui/tailwind.config.ts` | Create | Tailwind config with suzuran theme tokens |
| `ui/postcss.config.js` | Create | PostCSS for Tailwind |
| `ui/index.html` | Create | SPA entry point |
| `ui/src/main.tsx` | Create | React root, QueryClient, Router |
| `ui/src/theme/ThemeProvider.tsx` | Create | CSS variable injection from user theme |
| `ui/src/theme/tokens.ts` | Create | Default dark + light theme CSS variable maps |
| `ui/src/api/client.ts` | Create | Axios instance with base URL + CSRF |
| `ui/src/api/auth.ts` | Create | login, register, logout, me API calls |
| `ui/src/api/libraries.ts` | Create | library list API call |
| `ui/src/contexts/AuthContext.tsx` | Create | Current user context + useAuth hook |
| `ui/src/pages/LoginPage.tsx` | Create | Login form |
| `ui/src/pages/RegisterPage.tsx` | Create | Register form (first-time setup) |
| `ui/src/components/TopNav.tsx` | Create | Top nav bar (Library · Inbox · Issues · Jobs · Settings) |
| `ui/src/components/LibraryTree.tsx` | Create | Left pane: library/artist tree skeleton |
| `ui/src/pages/LibraryPage.tsx` | Create | Two-pane layout with LibraryTree + track list placeholder |
| `ui/src/App.tsx` | Create | Routes: login, register, library (protected) |
| `src/app.rs` | Modify | Add `ServeDir` fallback for SPA |
| `Dockerfile` | Modify | Wire Stage 2 to actually run `npm ci && npm run build` |

---

## Task 1: Vite + React project scaffold

**Files:**
- Create: `ui/package.json`
- Create: `ui/vite.config.ts`
- Create: `ui/index.html`
- Create: `ui/tailwind.config.ts`
- Create: `ui/postcss.config.js`

- [ ] **Step 1: Write `ui/package.json`**

```json
{
  "name": "suzuran-ui",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router-dom": "^6.27.0",
    "@tanstack/react-query": "^5.59.0",
    "axios": "^1.7.7",
    "clsx": "^2.1.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.3",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.47",
    "tailwindcss": "^3.4.14",
    "typescript": "^5.6.3",
    "vite": "^5.4.10"
  }
}
```

- [ ] **Step 2: Write `ui/vite.config.ts`**

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
})
```

- [ ] **Step 3: Write `ui/tailwind.config.ts`**

```typescript
import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // suzuran design tokens
        bg: {
          base:    'var(--bg-base)',
          surface: 'var(--bg-surface)',
          panel:   'var(--bg-panel)',
          hover:   'var(--bg-hover)',
        },
        border: {
          DEFAULT: 'var(--border)',
          subtle:  'var(--border-subtle)',
        },
        text: {
          primary:   'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          muted:     'var(--text-muted)',
        },
        accent: {
          DEFAULT: 'var(--accent)',
          muted:   'var(--accent-muted)',
        },
        destructive: 'var(--destructive)',
        success:     'var(--success)',
      },
      fontFamily: {
        sans: ['Inter', 'Geist', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
} satisfies Config
```

- [ ] **Step 4: Write `ui/postcss.config.js`**

```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
```

- [ ] **Step 5: Write `ui/index.html`**

```html
<!DOCTYPE html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>suzuran</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 6: Create `ui/src/` directory skeleton**

```bash
mkdir -p ui/src/api ui/src/components ui/src/contexts ui/src/pages ui/src/theme
```

- [ ] **Step 7: Install dependencies**

```bash
cd ui && npm install
```

- [ ] **Step 8: Commit**

```bash
cd ..
git add ui/package.json ui/package-lock.json ui/vite.config.ts ui/tailwind.config.ts ui/postcss.config.js ui/index.html
git commit -m "chore: Vite + React + Tailwind scaffold in ui/"
```

---

## Task 2: ThemeProvider and tokens

**Files:**
- Create: `ui/src/theme/tokens.ts`
- Create: `ui/src/theme/ThemeProvider.tsx`

- [ ] **Step 1: Write `ui/src/theme/tokens.ts`**

```typescript
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
```

- [ ] **Step 2: Write `ui/src/theme/ThemeProvider.tsx`**

```tsx
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
```

---

## Task 3: API client and auth API

**Files:**
- Create: `ui/src/api/client.ts`
- Create: `ui/src/api/auth.ts`
- Create: `ui/src/api/libraries.ts`

- [ ] **Step 1: Write `ui/src/api/client.ts`**

```typescript
import axios from 'axios'

const client = axios.create({
  baseURL: '/api/v1',
  withCredentials: true, // send HttpOnly session cookie
})

export default client
```

- [ ] **Step 2: Write `ui/src/api/auth.ts`**

```typescript
import client from './client'

export interface User {
  id: number
  username: string
  email: string
  role: string
  display_name: string | null
}

export interface LoginResult {
  two_factor_required?: boolean
  token?: string
}

export async function register(
  username: string,
  email: string,
  password: string,
): Promise<User> {
  const res = await client.post<User>('/auth/register', { username, email, password })
  return res.data
}

export async function login(
  username: string,
  password: string,
): Promise<LoginResult> {
  const res = await client.post<LoginResult>('/auth/login', { username, password })
  return res.data ?? {}
}

export async function logout(): Promise<void> {
  await client.post('/auth/logout')
}

export async function getMe(): Promise<User> {
  const res = await client.get<User>('/auth/me')
  return res.data
}
```

- [ ] **Step 3: Write `ui/src/api/libraries.ts`**

```typescript
import client from './client'

export interface Library {
  id: number
  name: string
  root_path: string
  format: string
  parent_library_id: number | null
  scan_enabled: boolean
}

export async function listLibraries(): Promise<Library[]> {
  const res = await client.get<Library[]>('/libraries/')
  return res.data
}
```

---

## Task 4: AuthContext

**Files:**
- Create: `ui/src/contexts/AuthContext.tsx`

- [ ] **Step 1: Write `ui/src/contexts/AuthContext.tsx`**

```tsx
import {
  createContext,
  useContext,
  useState,
  useEffect,
  type ReactNode,
} from 'react'
import { getMe, logout as apiLogout, type User } from '../api/auth'

interface AuthContextValue {
  user: User | null
  loading: boolean
  setUser: (u: User | null) => void
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue>({
  user: null,
  loading: true,
  setUser: () => {},
  logout: async () => {},
})

export function useAuth() {
  return useContext(AuthContext)
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    getMe()
      .then(setUser)
      .catch(() => setUser(null))
      .finally(() => setLoading(false))
  }, [])

  const logout = async () => {
    await apiLogout()
    setUser(null)
  }

  return (
    <AuthContext.Provider value={{ user, loading, setUser, logout }}>
      {children}
    </AuthContext.Provider>
  )
}
```

---

## Task 5: Pages — Login and Register

**Files:**
- Create: `ui/src/pages/LoginPage.tsx`
- Create: `ui/src/pages/RegisterPage.tsx`

- [ ] **Step 1: Write `ui/src/pages/LoginPage.tsx`**

```tsx
import { useState, type FormEvent } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { login } from '../api/auth'
import { useAuth } from '../contexts/AuthContext'
import { getMe } from '../api/auth'

export function LoginPage() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const { setUser } = useAuth()

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const result = await login(username, password)
      if (result.two_factor_required) {
        // TODO Phase 1.5 UI: redirect to 2FA page
        setError('2FA required — 2FA UI not yet implemented')
        return
      }
      const me = await getMe()
      setUser(me)
      navigate('/')
    } catch (err: any) {
      setError(err.response?.data?.error ?? 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-bg-base">
      <div className="w-full max-w-sm bg-bg-surface border border-border rounded p-8">
        <h1 className="text-text-primary text-xl font-semibold mb-6 tracking-tight">
          suzuran
        </h1>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
              Username
            </label>
            <input
              type="text"
              value={username}
              onChange={e => setUsername(e.target.value)}
              required
              className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent"
            />
          </div>
          <div>
            <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
              Password
            </label>
            <input
              type="password"
              value={password}
              onChange={e => setPassword(e.target.value)}
              required
              className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent"
            />
          </div>
          {error && (
            <p className="text-destructive text-xs">{error}</p>
          )}
          <button
            type="submit"
            disabled={loading}
            className="w-full bg-accent text-white text-sm py-2 rounded hover:opacity-90 disabled:opacity-50"
          >
            {loading ? 'Signing in…' : 'Sign in'}
          </button>
        </form>
        <p className="text-text-muted text-xs mt-4 text-center">
          No account?{' '}
          <Link to="/register" className="text-accent hover:underline">
            Register
          </Link>
        </p>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Write `ui/src/pages/RegisterPage.tsx`**

```tsx
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { register, login } from '../api/auth'
import { useAuth } from '../contexts/AuthContext'
import { getMe } from '../api/auth'

export function RegisterPage() {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const { setUser } = useAuth()

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await register(username, email, password)
      await login(username, password)
      const me = await getMe()
      setUser(me)
      navigate('/')
    } catch (err: any) {
      setError(err.response?.data?.error ?? 'Registration failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-bg-base">
      <div className="w-full max-w-sm bg-bg-surface border border-border rounded p-8">
        <h1 className="text-text-primary text-xl font-semibold mb-2 tracking-tight">
          suzuran
        </h1>
        <p className="text-text-muted text-xs mb-6">Create your account</p>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
              Username
            </label>
            <input
              type="text"
              value={username}
              onChange={e => setUsername(e.target.value)}
              required
              className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent"
            />
          </div>
          <div>
            <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
              Email
            </label>
            <input
              type="email"
              value={email}
              onChange={e => setEmail(e.target.value)}
              required
              className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent"
            />
          </div>
          <div>
            <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
              Password
            </label>
            <input
              type="password"
              value={password}
              onChange={e => setPassword(e.target.value)}
              required
              minLength={8}
              className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent"
            />
          </div>
          {error && (
            <p className="text-destructive text-xs">{error}</p>
          )}
          <button
            type="submit"
            disabled={loading}
            className="w-full bg-accent text-white text-sm py-2 rounded hover:opacity-90 disabled:opacity-50"
          >
            {loading ? 'Creating account…' : 'Create account'}
          </button>
        </form>
      </div>
    </div>
  )
}
```

---

## Task 6: TopNav, LibraryTree, LibraryPage

**Files:**
- Create: `ui/src/components/TopNav.tsx`
- Create: `ui/src/components/LibraryTree.tsx`
- Create: `ui/src/pages/LibraryPage.tsx`

- [ ] **Step 1: Write `ui/src/components/TopNav.tsx`**

```tsx
import { NavLink } from 'react-router-dom'
import { useAuth } from '../contexts/AuthContext'

export function TopNav() {
  const { user, logout } = useAuth()

  const navItem = (to: string, label: string) => (
    <NavLink
      to={to}
      className={({ isActive }) =>
        `text-xs px-3 py-2 border-b-2 transition-colors ${
          isActive
            ? 'text-accent border-accent'
            : 'text-text-muted border-transparent hover:text-text-secondary'
        }`
      }
    >
      {label}
    </NavLink>
  )

  return (
    <header className="flex items-center gap-0 bg-bg-base border-b border-border px-4 flex-shrink-0 h-9">
      <span className="text-accent text-sm font-semibold mr-4 tracking-tight">
        suzuran
      </span>
      {navItem('/', 'Library')}
      {navItem('/inbox', 'Inbox')}
      {navItem('/issues', 'Issues')}
      {navItem('/jobs', 'Jobs')}
      <div className="ml-auto flex items-center gap-3">
        <NavLink
          to="/settings"
          className="text-xs text-text-muted hover:text-text-secondary"
        >
          Settings
        </NavLink>
        {user && (
          <button
            onClick={logout}
            className="text-xs text-text-muted hover:text-text-secondary"
          >
            {user.username}
          </button>
        )}
      </div>
    </header>
  )
}
```

- [ ] **Step 2: Write `ui/src/components/LibraryTree.tsx`**

```tsx
import { useQuery } from '@tanstack/react-query'
import { listLibraries, type Library } from '../api/libraries'

export function LibraryTree() {
  const { data: libraries = [], isLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  if (isLoading) {
    return (
      <div className="p-3 text-text-muted text-xs">Loading…</div>
    )
  }

  return (
    <div className="flex flex-col overflow-y-auto text-xs">
      <div className="px-2 py-1 mb-1 border-b border-border-subtle">
        <input
          type="search"
          placeholder="Search…"
          className="w-full bg-bg-base border border-border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent"
        />
      </div>
      {libraries.map(lib => (
        <LibraryNode key={lib.id} library={lib} />
      ))}
    </div>
  )
}

function LibraryNode({ library }: { library: Library }) {
  return (
    <div>
      <div className="px-2 py-0.5 text-accent bg-accent-muted border-l-2 border-accent flex justify-between items-center cursor-pointer">
        <span>▾ {library.name}</span>
        <span className="text-text-muted uppercase text-[9px] tracking-wider">
          {library.format}
        </span>
      </div>
      <div className="pl-4 py-0.5 text-text-secondary cursor-pointer hover:bg-bg-hover">
        ▾ Artists
      </div>
      <div className="pl-4 py-0.5 text-text-muted cursor-pointer hover:bg-bg-hover">
        ▸ Albums
      </div>
      <div className="pl-4 py-0.5 text-text-muted cursor-pointer hover:bg-bg-hover">
        ▸ Genres
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Write `ui/src/pages/LibraryPage.tsx`**

```tsx
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'

export function LibraryPage() {
  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-1 overflow-hidden">
        {/* Left: tree pane */}
        <aside className="w-44 flex-shrink-0 bg-bg-panel border-r border-border overflow-y-auto">
          <LibraryTree />
        </aside>

        {/* Right: track list */}
        <main className="flex flex-col flex-1 overflow-hidden">
          {/* Toolbar */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
            <span className="text-text-muted text-xs">Select a library</span>
            <div className="ml-auto flex gap-1">
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Group: None ▾
              </button>
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Sort ▾
              </button>
            </div>
          </div>

          {/* Column headers */}
          <div className="flex items-center gap-0 px-2 py-1 bg-bg-panel border-b border-border text-text-muted text-[9px] uppercase tracking-wider flex-shrink-0">
            <span className="w-5"></span>
            <span className="w-6">#</span>
            <span className="flex-[3]">Title</span>
            <span className="flex-[2]">Artist</span>
            <span className="flex-[2]">Album</span>
            <span className="w-10">Year</span>
            <span className="flex-1">Genre</span>
            <span className="w-12">Format</span>
            <span className="w-14">Bitrate</span>
            <span className="w-10">Time</span>
            <span className="w-6 text-accent cursor-pointer" title="Customize columns">⊕</span>
          </div>

          {/* Track list area */}
          <div className="flex-1 overflow-y-auto">
            <div className="flex items-center justify-center h-32 text-text-muted text-xs">
              Select a library from the tree to view tracks.
            </div>
          </div>
        </main>
      </div>
    </div>
  )
}
```

---

## Task 7: App router and main entry

**Files:**
- Create: `ui/src/App.tsx`
- Create: `ui/src/main.tsx`
- Create: `ui/src/index.css`

- [ ] **Step 1: Write `ui/src/index.css`**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

* {
  box-sizing: border-box;
}

html, body, #root {
  height: 100%;
  margin: 0;
  padding: 0;
}

body {
  font-family: Inter, Geist, system-ui, sans-serif;
  -webkit-font-smoothing: antialiased;
}
```

- [ ] **Step 2: Write `ui/src/App.tsx`**

```tsx
import { Navigate, Route, Routes } from 'react-router-dom'
import { useAuth } from './contexts/AuthContext'
import { LoginPage } from './pages/LoginPage'
import { RegisterPage } from './pages/RegisterPage'
import { LibraryPage } from './pages/LibraryPage'

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth()
  if (loading) return null
  if (!user) return <Navigate to="/login" replace />
  return <>{children}</>
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route
        path="/*"
        element={
          <ProtectedRoute>
            <LibraryPage />
          </ProtectedRoute>
        }
      />
    </Routes>
  )
}
```

- [ ] **Step 3: Write `ui/src/main.tsx`**

```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ThemeProvider } from './theme/ThemeProvider'
import { AuthProvider } from './contexts/AuthContext'
import { App } from './App'
import './index.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 30_000 },
  },
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider initialBase="dark">
        <BrowserRouter>
          <AuthProvider>
            <App />
          </AuthProvider>
        </BrowserRouter>
      </ThemeProvider>
    </QueryClientProvider>
  </StrictMode>,
)
```

- [ ] **Step 4: Add `tsconfig.json` and `tsconfig.node.json`**

```bash
cd ui
cat > tsconfig.json << 'EOF'
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
EOF
cat > tsconfig.node.json << 'EOF'
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
EOF
cd ..
```

- [ ] **Step 5: Test build**

```bash
cd ui && npm run build 2>&1 | tail -10
```

Expected: `dist/index.html` created, no TypeScript errors.

- [ ] **Step 6: Commit**

```bash
cd ..
git add ui/src/ ui/tsconfig.json ui/tsconfig.node.json
git commit -m "feat: React SPA — ThemeProvider, auth pages, TopNav, LibraryPage skeleton"
```

---

## Task 8: Serve UI from Axum + update Dockerfile

**Files:**
- Modify: `src/app.rs`
- Modify: `Cargo.toml`
- Modify: `Dockerfile`

- [ ] **Step 1: Add `tower-http` ServeDir feature to Cargo.toml**

Update the tower-http dependency:

```toml
tower-http = { version = "0.5", features = ["trace", "fs"] }
```

- [ ] **Step 2: Update `src/app.rs` to serve `ui/dist` with SPA fallback**

```rust
use axum::{routing::get, Router};
use tower_http::services::{ServeDir, ServeFile};

use crate::{api::api_router, error::AppError, state::AppState};
use axum::{extract::State, Json};
use serde_json::{json, Value};

pub fn build_router(state: AppState) -> Router {
    // Serve the compiled SPA — fallback to index.html for client-side routing
    let ui_service = ServeDir::new("ui/dist")
        .not_found_service(ServeFile::new("ui/dist/index.html"));

    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_router(state.clone()))
        .fallback_service(ui_service)
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    state.db.health_check().await?;
    Ok(Json(json!({ "status": "ok" })))
}
```

- [ ] **Step 3: Update Dockerfile Stage 2 to actually build the UI**

Replace the placeholder ui-builder stage:

```dockerfile
# Stage 2: UI build
FROM node:20-slim AS ui-builder
WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build
```

- [ ] **Step 4: Build the full Docker image**

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: All 3 stages build. UI dist files present in the final image.

- [ ] **Step 5: Start and verify UI is served**

```bash
docker compose up --build -d
sleep 5
curl -sf http://localhost:3000/ | grep -c "suzuran"
```

Expected: output `> 0` (the index.html contains "suzuran").

- [ ] **Step 6: Open in browser to verify the login page renders**

Navigate to `http://localhost:3000` — should show the suzuran login page with dark theme.

- [ ] **Step 7: Tear down**

```bash
docker compose down -v
```

- [ ] **Step 8: Commit**

```bash
git add src/app.rs Cargo.toml Dockerfile tasks/codebase-filemap.md
git commit -m "feat: serve compiled SPA from Axum via ServeDir; Dockerfile Stage 2 builds UI"
```

---

## Task 9: Dev workflow note

- [ ] **Step 1: Confirm dev workflow works**

```bash
# Terminal 1: start backend (requires running DB)
docker compose up -d db
DATABASE_URL=postgres://suzuran:suzuran@localhost:5432/suzuran \
  JWT_SECRET=dev-secret cargo run

# Terminal 2: start Vite dev server (proxies /api to localhost:3000)
cd ui && npm run dev
```

Navigate to `http://localhost:5173` for hot-reload dev experience.

- [ ] **Step 2: Add `.gitignore` entries for UI build artifacts**

Ensure `ui/node_modules/` and `ui/dist/` are in `.gitignore` (they should already be from Phase 1.1's `.gitignore`). Verify:

```bash
git check-ignore ui/node_modules ui/dist
```

Expected: both lines output (both are ignored).

- [ ] **Step 3: Final commit**

```bash
git add .
git status  # confirm only expected files
git commit -m "docs: Phase 1.10 complete — UI shell with ThemeProvider, auth, library skeleton"
```
