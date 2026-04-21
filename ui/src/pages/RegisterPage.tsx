import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQueryClient } from '@tanstack/react-query'
import { register, login, getMe } from '../api/auth'
import { useAuth } from '../contexts/AuthContext'

interface RegisterPageProps {
  setupMode?: boolean
}

export function RegisterPage({ setupMode = false }: RegisterPageProps) {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const { setUser } = useAuth()
  const queryClient = useQueryClient()

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await register(username, email, password)
      await login(username, password)
      const me = await getMe()
      setUser(me)
      // Mark setup as complete so routing guard switches to normal mode
      queryClient.setQueryData(['setup-status'], { needs_setup: false })
      navigate('/')
    } catch (err: unknown) {
      const e = err as { response?: { data?: { error?: string } } }
      setError(e.response?.data?.error ?? 'Registration failed')
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
        <p className="text-text-muted text-xs mb-6">
          {setupMode
            ? 'First-time setup — create the admin account'
            : 'Create your account'}
        </p>
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
              autoComplete="username"
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
              autoComplete="email"
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
              autoComplete="new-password"
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
            {loading
              ? (setupMode ? 'Setting up…' : 'Creating account…')
              : (setupMode ? 'Create admin account' : 'Create account')}
          </button>
        </form>
      </div>
    </div>
  )
}
