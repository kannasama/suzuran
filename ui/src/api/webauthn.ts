import client from './client'

export interface CredentialInfo {
  id: number
  name: string
  created_at: string
  last_used_at: string | null
}

export async function listCredentials(): Promise<CredentialInfo[]> {
  const res = await client.get<CredentialInfo[]>('/webauthn/credentials')
  return res.data
}

export async function registrationChallenge(): Promise<PublicKeyCredentialCreationOptions> {
  const res = await client.post<PublicKeyCredentialCreationOptions>('/webauthn/register/challenge')
  return res.data
}

export async function completeRegistration(name: string, response: unknown): Promise<void> {
  await client.post('/webauthn/register/complete', { name, response })
}

export async function deleteCredential(id: number): Promise<void> {
  await client.delete(`/webauthn/credentials/${id}`)
}

export async function authChallenge(token: string): Promise<PublicKeyCredentialRequestOptions> {
  const res = await client.post<PublicKeyCredentialRequestOptions>(
    '/webauthn/authenticate/challenge',
    { token },
  )
  return res.data
}

export async function completeAuth(token: string, response: unknown): Promise<void> {
  await client.post('/webauthn/authenticate/complete', { token, response })
}

// Serialize a PublicKeyCredential response so it can be sent as JSON.
// Converts ArrayBuffers to base64url strings.
export function serializeCredential(cred: PublicKeyCredential): unknown {
  const ab2b64 = (buf: ArrayBuffer) =>
    btoa(String.fromCharCode(...new Uint8Array(buf)))
      .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')

  const resp = cred.response
  if (resp instanceof AuthenticatorAttestationResponse) {
    return {
      id: cred.id,
      rawId: ab2b64(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: ab2b64(resp.clientDataJSON),
        attestationObject: ab2b64(resp.attestationObject),
      },
    }
  }
  if (resp instanceof AuthenticatorAssertionResponse) {
    return {
      id: cred.id,
      rawId: ab2b64(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: ab2b64(resp.clientDataJSON),
        authenticatorData: ab2b64(resp.authenticatorData),
        signature: ab2b64(resp.signature),
        userHandle: resp.userHandle ? ab2b64(resp.userHandle) : null,
      },
    }
  }
  return cred
}

// Decode a challenge from base64url to Uint8Array (for use in navigator.credentials calls).
export function decodeBase64Url(s: string): Uint8Array {
  const b64 = s.replace(/-/g, '+').replace(/_/g, '/').padEnd(
    s.length + ((4 - (s.length % 4)) % 4), '='
  )
  return Uint8Array.from(atob(b64), c => c.charCodeAt(0))
}
