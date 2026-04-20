import client from './client'
import type { EncodingProfile, UpsertEncodingProfile } from '../types/encodingProfile'

export async function listEncodingProfiles(): Promise<EncodingProfile[]> {
  const res = await client.get<EncodingProfile[]>('/encoding-profiles')
  return res.data
}

export async function createEncodingProfile(data: UpsertEncodingProfile): Promise<EncodingProfile> {
  const res = await client.post<EncodingProfile>('/encoding-profiles', data)
  return res.data
}

export async function updateEncodingProfile(id: number, data: UpsertEncodingProfile): Promise<EncodingProfile> {
  const res = await client.put<EncodingProfile>(`/encoding-profiles/${id}`, data)
  return res.data
}

export async function deleteEncodingProfile(id: number): Promise<void> {
  await client.delete(`/encoding-profiles/${id}`)
}
