import client from './client'

export interface User {
  id: number
  username: string
  email: string
  role: string
  display_name: string | null
}

export interface LoginResult {
  two_factor_required?: boolean
  token?: string
}

export async function register(
  username: string,
  email: string,
  password: string,
): Promise<User> {
  const res = await client.post<User>('/auth/register', { username, email, password })
  return res.data
}

export async function login(
  username: string,
  password: string,
): Promise<LoginResult> {
  const res = await client.post<LoginResult>('/auth/login', { username, password })
  return res.data ?? {}
}

export async function logout(): Promise<void> {
  await client.post('/auth/logout')
}

export async function getMe(): Promise<User> {
  const res = await client.get<User>('/auth/me')
  return res.data
}
