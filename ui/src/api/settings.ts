import client from './client'

export interface Setting {
  key: string
  value: string
  updated_at: string
}

export async function listSettings(): Promise<Setting[]> {
  const res = await client.get<Setting[]>('/settings')
  return res.data
}

export async function setSetting(key: string, value: string): Promise<Setting> {
  const res = await client.put<Setting>(`/settings/${key}`, { value })
  return res.data
}
