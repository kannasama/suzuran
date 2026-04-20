import client from './client'
import type { ArtProfile, UpsertArtProfile } from '../types/artProfile'

export async function listArtProfiles(): Promise<ArtProfile[]> {
  const res = await client.get<ArtProfile[]>('/art-profiles')
  return res.data
}

export async function createArtProfile(data: UpsertArtProfile): Promise<ArtProfile> {
  const res = await client.post<ArtProfile>('/art-profiles', data)
  return res.data
}

export async function updateArtProfile(id: number, data: UpsertArtProfile): Promise<ArtProfile> {
  const res = await client.put<ArtProfile>(`/art-profiles/${id}`, data)
  return res.data
}

export async function deleteArtProfile(id: number): Promise<void> {
  await client.delete(`/art-profiles/${id}`)
}
