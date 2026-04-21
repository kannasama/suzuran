import { useState, useEffect, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import QRCode from 'qrcode'
import { TopNav } from '../components/TopNav'
import {
  getTotpStatus,
  enrollTotp,
  verifyTotp,
  disenrollTotp,
} from '../api/totp'
import {
  listCredentials,
  registrationChallenge,
  completeRegistration,
  deleteCredential,
  serializeCredential,
  decodeBase64Url,
  type CredentialInfo,
} from '../api/webauthn'

export default function AccountPage() {
  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <main className="flex-1 overflow-y-auto">
        <div className="max-w-lg mx-auto p-6 space-y-8">
          <h1 className="text-text-primary font-semibold text-sm">Account Security</h1>
          <TotpSection />
          <PasskeysSection />
        </div>
      </main>
    </div>
  )
}

// ---------------------------------------------------------------------------
// TOTP section
// ---------------------------------------------------------------------------

function TotpQrCode({ uri }: { uri: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [showSecret, setShowSecret] = useState(false)

  // Extract the secret from the otpauth URI for manual entry fallback
  const secret = (() => {
    try {
      const url = new URL(uri)
      return url.searchParams.get('secret') ?? null
    } catch { return null }
  })()

  useEffect(() => {
    if (!canvasRef.current) return
    QRCode.toCanvas(canvasRef.current, uri, {
      width: 180,
      margin: 2,
      color: { dark: '#000000', light: '#ffffff' },
    }).catch(() => {/* ignore render errors */})
  }, [uri])

  return (
    <div className="space-y-2">
      <canvas
        ref={canvasRef}
        className="rounded border border-border bg-white"
        style={{ imageRendering: 'pixelated' }}
      />
      <button
        type="button"
        onClick={() => setShowSecret(v => !v)}
        className="text-xs text-text-muted hover:text-text-secondary transition-colors"
      >
        {showSecret ? 'Hide secret' : 'Can\'t scan? Show secret key'}
      </button>
      {showSecret && secret && (
        <code className="block bg-bg-panel border border-border rounded px-3 py-2 text-xs text-text-primary break-all font-mono tracking-wider">
          {secret}
        </code>
      )}
    </div>
  )
}

function TotpSection() {
  const qc = useQueryClient()
  const [enrolling, setEnrolling] = useState(false)
  const [otpauthUri, setOtpauthUri] = useState<string | null>(null)
  const [code, setCode] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  const { data: status, isLoading } = useQuery({
    queryKey: ['totp-status'],
    queryFn: getTotpStatus,
  })

  const startEnroll = useMutation({
    mutationFn: enrollTotp,
    onSuccess: (data) => {
      setOtpauthUri(data.otpauth_uri)
      setEnrolling(true)
      setError(null)
    },
    onError: () => setError('Failed to start enrollment. Please try again.'),
  })

  const confirmEnroll = useMutation({
    mutationFn: () => verifyTotp(code),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['totp-status'] })
      setEnrolling(false)
      setOtpauthUri(null)
      setCode('')
      setSuccess('Authenticator app enabled.')
      setTimeout(() => setSuccess(null), 3000)
    },
    onError: () => setError('Invalid code — please try again.'),
  })

  const disenroll = useMutation({
    mutationFn: disenrollTotp,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['totp-status'] })
      setSuccess('Authenticator app removed.')
      setTimeout(() => setSuccess(null), 3000)
    },
    onError: () => setError('Failed to remove authenticator. Please try again.'),
  })

  if (isLoading) return (
    <Section title="Authenticator App (TOTP)">
      <p className="text-text-muted text-xs">Loading…</p>
    </Section>
  )

  return (
    <Section title="Authenticator App (TOTP)">
      {success && <p className="text-xs text-success mb-3">{success}</p>}
      {error && <p className="text-xs text-destructive mb-3">{error}</p>}

      {!status?.enrolled && !enrolling && (
        <div className="space-y-2">
          <p className="text-text-secondary text-xs">
            Use an authenticator app (e.g. Aegis, Google Authenticator) as a second factor when signing in.
          </p>
          <button
            onClick={() => { setError(null); startEnroll.mutate() }}
            disabled={startEnroll.isPending}
            className="text-xs bg-accent text-bg-base rounded px-3 py-1.5 font-medium hover:opacity-90 disabled:opacity-40"
          >
            {startEnroll.isPending ? 'Setting up…' : 'Set up authenticator'}
          </button>
        </div>
      )}

      {enrolling && otpauthUri && (
        <div className="space-y-3">
          <p className="text-text-secondary text-xs">
            Scan the QR code with your authenticator app (e.g. Aegis, Google Authenticator):
          </p>
          <TotpQrCode uri={otpauthUri} />
          <p className="text-text-secondary text-xs">
            Then enter the 6-digit code from your app to confirm:
          </p>
          <div className="flex gap-2 items-center">
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              maxLength={6}
              value={code}
              onChange={e => { setCode(e.target.value.replace(/\D/g, '')); setError(null) }}
              placeholder="000000"
              autoFocus
              className="w-28 bg-bg-panel border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono tracking-widest"
            />
            <button
              onClick={() => confirmEnroll.mutate()}
              disabled={code.length !== 6 || confirmEnroll.isPending}
              className="text-xs bg-accent text-bg-base rounded px-3 py-1.5 font-medium hover:opacity-90 disabled:opacity-40"
            >
              {confirmEnroll.isPending ? 'Verifying…' : 'Confirm'}
            </button>
            <button
              onClick={() => { setEnrolling(false); setOtpauthUri(null); setCode(''); setError(null) }}
              className="text-xs text-text-muted hover:text-text-secondary transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {status?.enrolled && !enrolling && (
        <div className="space-y-2">
          <p className="text-text-secondary text-xs">
            Authenticator app is enabled. You will be prompted for a code when signing in.
          </p>
          <button
            onClick={() => { setError(null); disenroll.mutate() }}
            disabled={disenroll.isPending}
            className="text-xs border border-border text-destructive rounded px-3 py-1.5 hover:bg-bg-hover transition-colors disabled:opacity-40"
          >
            {disenroll.isPending ? 'Removing…' : 'Remove authenticator'}
          </button>
        </div>
      )}
    </Section>
  )
}

// ---------------------------------------------------------------------------
// Passkeys (WebAuthn) section
// ---------------------------------------------------------------------------

function PasskeysSection() {
  const qc = useQueryClient()
  const [addingName, setAddingName] = useState('')
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  const { data: credentials = [], isLoading } = useQuery({
    queryKey: ['webauthn-credentials'],
    queryFn: listCredentials,
  })

  const removeCred = useMutation({
    mutationFn: (id: number) => deleteCredential(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['webauthn-credentials'] }),
    onError: () => setError('Failed to remove passkey.'),
  })

  const webAuthnSupported = typeof window !== 'undefined' && !!window.PublicKeyCredential

  async function handleAddPasskey() {
    if (!addingName.trim()) return
    setError(null)
    setAdding(true)
    try {
      // webauthn-rs returns { publicKey: { challenge, user, ... } }
      // Decode the base64url fields within publicKey before handing to the browser.
      const challengeResponse = await registrationChallenge()
      const wrapper = challengeResponse as unknown as Record<string, unknown>
      const pk = wrapper.publicKey as Record<string, unknown>
      if (pk.challenge && typeof pk.challenge === 'string') {
        pk.challenge = decodeBase64Url(pk.challenge).buffer
      }
      if (pk.user && typeof pk.user === 'object') {
        const u = pk.user as Record<string, unknown>
        if (u.id && typeof u.id === 'string') {
          u.id = decodeBase64Url(u.id).buffer
        }
      }
      const cred = await navigator.credentials.create(
        wrapper as unknown as CredentialCreationOptions
      ) as PublicKeyCredential
      await completeRegistration(addingName.trim(), serializeCredential(cred))
      qc.invalidateQueries({ queryKey: ['webauthn-credentials'] })
      setAddingName('')
      setSuccess('Passkey added.')
      setTimeout(() => setSuccess(null), 3000)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Registration failed.'
      setError(msg)
    } finally {
      setAdding(false)
    }
  }

  return (
    <Section title="Passkeys">
      {success && <p className="text-xs text-success mb-3">{success}</p>}
      {error && <p className="text-xs text-destructive mb-3">{error}</p>}

      {!webAuthnSupported && (
        <p className="text-text-muted text-xs">
          Your browser does not support passkeys (WebAuthn).
        </p>
      )}

      {webAuthnSupported && (
        <>
          {isLoading ? (
            <p className="text-text-muted text-xs">Loading…</p>
          ) : credentials.length === 0 ? (
            <p className="text-text-secondary text-xs mb-3">No passkeys registered.</p>
          ) : (
            <div className="flex flex-col gap-1 mb-4">
              {credentials.map(c => (
                <CredentialRow key={c.id} cred={c} onDelete={() => removeCred.mutate(c.id)} deleting={removeCred.isPending && removeCred.variables === c.id} />
              ))}
            </div>
          )}

          <div className="flex gap-2 items-center">
            <input
              type="text"
              value={addingName}
              onChange={e => setAddingName(e.target.value)}
              placeholder="Passkey name (e.g. YubiKey 5)"
              className="flex-1 bg-bg-panel border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            />
            <button
              onClick={handleAddPasskey}
              disabled={adding || !addingName.trim()}
              className="text-xs bg-accent text-bg-base rounded px-3 py-1.5 font-medium hover:opacity-90 disabled:opacity-40 shrink-0"
            >
              {adding ? 'Registering…' : 'Add passkey'}
            </button>
          </div>
          <p className="text-text-muted text-xs mt-1">
            Use a hardware key, device biometric, or platform authenticator.
          </p>
        </>
      )}
    </Section>
  )
}

function CredentialRow({ cred, onDelete, deleting }: { cred: CredentialInfo; onDelete: () => void; deleting: boolean }) {
  const createdAt = new Date(cred.created_at).toLocaleDateString()
  const lastUsed = cred.last_used_at ? new Date(cred.last_used_at).toLocaleDateString() : 'Never'
  return (
    <div className="flex items-center justify-between px-3 py-2 bg-bg-panel border border-border rounded">
      <div>
        <span className="text-text-primary text-xs font-medium">{cred.name}</span>
        <span className="text-text-muted text-xs ml-3">Added {createdAt}</span>
        <span className="text-text-muted text-xs ml-3">Last used {lastUsed}</span>
      </div>
      <button
        onClick={onDelete}
        disabled={deleting}
        className="text-xs text-destructive hover:opacity-70 transition-opacity disabled:opacity-40"
      >
        Remove
      </button>
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h2 className="text-text-muted text-xs uppercase tracking-wider mb-3">{title}</h2>
      <div className="bg-bg-surface border border-border rounded p-4">
        {children}
      </div>
    </div>
  )
}
