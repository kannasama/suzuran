import client from './client'
import type { VirtualLibrary, VirtualLibrarySource, UpsertVirtualLibrary } from '../types/virtualLibrary'

export async function listVirtualLibraries(): Promise<VirtualLibrary[]> {
  const res = await client.get<VirtualLibrary[]>('/virtual-libraries')
  return res.data
}

export async function createVirtualLibrary(data: UpsertVirtualLibrary): Promise<VirtualLibrary> {
  const res = await client.post<VirtualLibrary>('/virtual-libraries', data)
  return res.data
}

export async function updateVirtualLibrary(id: number, data: UpsertVirtualLibrary): Promise<VirtualLibrary> {
  const res = await client.put<VirtualLibrary>(`/virtual-libraries/${id}`, data)
  return res.data
}

export async function deleteVirtualLibrary(id: number): Promise<void> {
  await client.delete(`/virtual-libraries/${id}`)
}

export async function getSources(id: number): Promise<VirtualLibrarySource[]> {
  const res = await client.get<VirtualLibrarySource[]>(`/virtual-libraries/${id}/sources`)
  return res.data
}

export async function setSources(
  id: number,
  sources: Array<{ library_id: number; library_profile_id: number | null; priority: number }>,
): Promise<void> {
  await client.put(`/virtual-libraries/${id}/sources`, sources)
}

export async function triggerSync(id: number): Promise<void> {
  await client.post(`/virtual-libraries/${id}/sync`)
}
