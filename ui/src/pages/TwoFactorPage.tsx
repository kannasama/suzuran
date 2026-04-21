import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import client from '../api/client'
import { getMe } from '../api/auth'
import { useAuth } from '../contexts/AuthContext'
import { authChallenge, completeAuth, serializeCredential, decodeBase64Url } from '../api/webauthn'

export default function TwoFactorPage() {
  const navigate = useNavigate()
  const location = useLocation()
  const { setUser } = useAuth()
  const token = (location.state as { token?: string } | null)?.token ?? ''

  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(false)
  const [passkeyLoading, setPasskeyLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState<'totp' | 'passkey'>('totp')

  const webAuthnSupported = typeof window !== 'undefined' && !!window.PublicKeyCredential

  if (!token) {
    navigate('/login', { replace: true })
    return null
  }

  async function handleTotpSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await client.post('/totp/complete', { token, code })
      const me = await getMe()
      setUser(me)
      navigate('/', { replace: true })
    } catch {
      setError('Invalid code. Please try again.')
    } finally {
      setLoading(false)
    }
  }

  async function handlePasskeyAuth() {
    setError(null)
    setPasskeyLoading(true)
    try {
      const challengeOptions = await authChallenge(token)
      const options = challengeOptions as unknown as Record<string, unknown>
      if (options.challenge && typeof options.challenge === 'string') {
        options.challenge = decodeBase64Url(options.challenge).buffer
      }
      if (Array.isArray(options.allowCredentials)) {
        options.allowCredentials = (options.allowCredentials as Array<Record<string, unknown>>).map(c => ({
          ...c,
          id: typeof c.id === 'string' ? decodeBase64Url(c.id).buffer : c.id,
        }))
      }
      const cred = await navigator.credentials.get({
        publicKey: options as unknown as PublicKeyCredentialRequestOptions,
      }) as PublicKeyCredential
      await completeAuth(token, serializeCredential(cred))
      const me = await getMe()
      setUser(me)
      navigate('/', { replace: true })
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Passkey authentication failed.'
      setError(msg)
    } finally {
      setPasskeyLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-bg-base">
      <div className="w-full max-w-sm bg-bg-surface border border-border rounded p-8">
        <h1 className="text-text-primary text-xl font-semibold mb-1 tracking-tight">
          suzuran
        </h1>
        <p className="text-text-muted text-xs mb-6">Two-factor verification required</p>

        {webAuthnSupported && (
          <div className="flex gap-0 border-b border-border mb-5">
            <button
              onClick={() => setTab('totp')}
              className={`text-xs px-3 py-2 border-b-2 transition-colors -mb-px ${
                tab === 'totp' ? 'text-accent border-accent' : 'text-text-muted border-transparent hover:text-text-secondary'
              }`}
            >
              Authenticator app
            </button>
            <button
              onClick={() => setTab('passkey')}
              className={`text-xs px-3 py-2 border-b-2 transition-colors -mb-px ${
                tab === 'passkey' ? 'text-accent border-accent' : 'text-text-muted border-transparent hover:text-text-secondary'
              }`}
            >
              Passkey
            </button>
          </div>
        )}

        {error && <p className="text-destructive text-xs mb-3">{error}</p>}

        {tab === 'totp' && (
          <form onSubmit={handleTotpSubmit} className="space-y-4">
            <div>
              <label className="block text-text-secondary text-xs uppercase tracking-wider mb-1">
                6-digit code
              </label>
              <input
                type="text"
                inputMode="numeric"
                pattern="[0-9]*"
                maxLength={6}
                value={code}
                onChange={e => { setCode(e.target.value.replace(/\D/g, '')); setError(null) }}
                autoFocus
                className="w-full bg-bg-panel border border-border text-text-primary text-sm px-3 py-2 rounded focus:outline-none focus:border-accent font-mono tracking-widest"
              />
            </div>
            <button
              type="submit"
              disabled={loading || code.length !== 6}
              className="w-full bg-accent text-white text-sm py-2 rounded hover:opacity-90 disabled:opacity-50"
            >
              {loading ? 'Verifying…' : 'Verify'}
            </button>
          </form>
        )}

        {tab === 'passkey' && webAuthnSupported && (
          <div className="space-y-4">
            <p className="text-text-secondary text-xs">
              Use your registered passkey (hardware key or device biometric) to authenticate.
            </p>
            <button
              onClick={handlePasskeyAuth}
              disabled={passkeyLoading}
              className="w-full bg-accent text-white text-sm py-2 rounded hover:opacity-90 disabled:opacity-50"
            >
              {passkeyLoading ? 'Waiting for passkey…' : 'Authenticate with passkey'}
            </button>
          </div>
        )}

        <button
          onClick={() => navigate('/login', { replace: true })}
          className="mt-4 text-text-muted text-xs hover:text-text-secondary transition-colors w-full text-center"
        >
          Back to login
        </button>
      </div>
    </div>
  )
}
