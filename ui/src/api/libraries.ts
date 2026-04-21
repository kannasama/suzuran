import client from './client'

export interface Library {
  id: number
  name: string
  root_path: string
  format: string
  parent_library_id: number | null
  encoding_profile_id: number | null
  scan_enabled: boolean
  scan_interval_secs: number
  auto_transcode_on_ingest: boolean
  auto_organize_on_ingest: boolean
  tag_encoding: string
}

export interface CreateLibraryInput {
  name: string
  root_path: string
  format: string
  parent_library_id: number | null
}

export interface UpdateLibraryInput {
  name: string
  scan_enabled: boolean
  scan_interval_secs: number
  auto_transcode_on_ingest: boolean
  auto_organize_on_ingest: boolean
  tag_encoding: string
}

export async function listLibraries(): Promise<Library[]> {
  const res = await client.get<Library[]>('/libraries')
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
