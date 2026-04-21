import client from './client'

export function searchMb(body: { title: string; artist: string; album: string }) {
  return client
    .post<Array<{ tags: Record<string, string>; confidence: number }>>('/search/mb', body)
    .then(r => r.data)
}

export function searchFreedB(body: { disc_id?: string; artist?: string; album?: string }) {
  return client
    .post<Array<{ artist: string; album: string; year: string; genre: string; tracks: string[] }>>('/search/freedb', body)
    .then(r => r.data)
}
