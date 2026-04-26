import client from './client'
import type { LibraryProfile, UpsertLibraryProfile } from '../types/libraryProfile'

export async function listLibraryProfiles(libraryId: number): Promise<LibraryProfile[]> {
  const res = await client.get<LibraryProfile[]>('/library-profiles', {
    params: { library_id: libraryId },
  })
  return res.data
}

export async function createLibraryProfile(data: UpsertLibraryProfile): Promise<LibraryProfile> {
  const res = await client.post<LibraryProfile>('/library-profiles', data)
  return res.data
}

export async function updateLibraryProfile(id: number, data: UpsertLibraryProfile): Promise<LibraryProfile> {
  const res = await client.put<LibraryProfile>(`/library-profiles/${id}`, data)
  return res.data
}

export async function deleteLibraryProfile(id: number): Promise<void> {
  await client.delete(`/library-profiles/${id}`)
}

export async function enqueueProfileTranscodes(id: number): Promise<{ enqueued: number }> {
  const res = await client.post<{ enqueued: number }>(`/library-profiles/${id}/enqueue-transcode`)
  return res.data
}
