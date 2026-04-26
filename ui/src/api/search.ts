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

export interface MbReleaseSummary {
  mb_release_id: string
  album: string
  albumartist: string
  date: string
  label: string
  catalognumber: string
  totaltracks: number
  totaldiscs: number
  status: string
  release_type: string
  cover_art_url: string
}

export interface MbReleaseTrack {
  number: string
  position: number
  recording_id: string
}

export interface MbReleaseDisc {
  position: number
  track_count: number
  tracks: MbReleaseTrack[]
}

export interface MbReleaseFull extends MbReleaseSummary {
  discs: MbReleaseDisc[]
}

export function searchMbRelease(body: { artist: string; album: string }) {
  return client
    .post<MbReleaseSummary[]>('/search/mb-release', body)
    .then(r => r.data)
}

export function getMbRelease(id: string) {
  return client
    .get<MbReleaseFull>(`/search/mb-release/${id}`)
    .then(r => r.data)
}
