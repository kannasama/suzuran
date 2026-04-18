import { useState, type FormEvent } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { login, getMe } from '../api/auth'
import { useAuth } from '../contexts/AuthContext'

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
    } catch (err: unknown) {
      const e = err as { response?: { data?: { error?: string } } }
      setError(e.response?.data?.error ?? 'Login failed')
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
