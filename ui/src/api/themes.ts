import client from './client'

export interface Theme {
  id: number
  name: string
  css_vars: Record<string, string>
  accent_color: string | null
  background_url: string | null
}

export interface UpsertTheme {
  name: string
  css_vars: Record<string, string>
  accent_color: string | null
  background_url: string | null
}

export async function listThemes(): Promise<Theme[]> {
  const res = await client.get<Theme[]>('/themes')
  return res.data
}

export async function createTheme(data: UpsertTheme): Promise<Theme> {
  const res = await client.post<Theme>('/themes', data)
  return res.data
}

export async function updateTheme(id: number, data: UpsertTheme): Promise<Theme> {
  const res = await client.put<Theme>(`/themes/${id}`, data)
  return res.data
}

export async function deleteTheme(id: number): Promise<void> {
  await client.delete(`/themes/${id}`)
}
