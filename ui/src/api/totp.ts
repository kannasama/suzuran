import client from './client'

export interface TotpStatus {
  enrolled: boolean
}

export interface EnrollResponse {
  otpauth_uri: string
}

export async function getTotpStatus(): Promise<TotpStatus> {
  const res = await client.get<TotpStatus>('/totp/status')
  return res.data
}

export async function enrollTotp(): Promise<EnrollResponse> {
  const res = await client.post<EnrollResponse>('/totp/enroll')
  return res.data
}

export async function verifyTotp(code: string): Promise<void> {
  await client.post('/totp/verify', { code })
}

export async function disenrollTotp(): Promise<void> {
  await client.delete('/totp/disenroll')
}
