import client from './client'
import type { Track } from '../types/track'

export interface Library {
  id: number
  name: string
  root_path: string
  format: string
  scan_enabled: boolean
  scan_interval_secs: number
  auto_organize_on_ingest: boolean
  tag_encoding: string
  organization_rule_id: number | null
}

export interface CreateLibraryInput {
  name: string
  root_path: string
  format: string
  organization_rule_id?: number | null
}

export interface UpdateLibraryInput {
  name: string
  scan_enabled: boolean
  scan_interval_secs: number
  auto_organize_on_ingest: boolean
  tag_encoding: string
  organization_rule_id: number | null
}

export async function listLibraries(): Promise<Library[]> {
  const res = await client.get<Library[]>('/libraries')
  return res.data
}

export async function getLibrary(id: number): Promise<Library> {
  const res = await client.get<Library>(`/libraries/${id}`)
  return res.data
}

export async function createLibrary(input: CreateLibraryInput): Promise<Library> {
  const res = await client.post<Library>('/libraries', input)
  return res.data
}

export async function updateLibrary(id: number, input: UpdateLibraryInput): Promise<Library> {
  const res = await client.put<Library>(`/libraries/${id}`, input)
  return res.data
}

export async function deleteLibrary(id: number): Promise<void> {
  await client.delete(`/libraries/${id}`)
}

export async function listLibraryTracks(libraryId: number): Promise<Track[]> {
  const res = await client.get<Track[]>(`/libraries/${libraryId}/tracks`)
  return res.data
}
