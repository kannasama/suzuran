import client from './client'

export interface Library {
  id: number
  name: string
  root_path: string
  format: string
  parent_library_id: number | null
  scan_enabled: boolean
}

export async function listLibraries(): Promise<Library[]> {
  const res = await client.get<Library[]>('/libraries')
  return res.data
}
