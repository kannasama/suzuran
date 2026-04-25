import client from './client'

export interface UserPref {
  key: string
  value: string
}

export async function getUserPrefs(): Promise<UserPref[]> {
  const res = await client.get<UserPref[]>('/user/prefs')
  return res.data
}

export async function setUserPref(key: string, value: string): Promise<void> {
  await client.put(`/user/prefs/${encodeURIComponent(key)}`, { value })
}
