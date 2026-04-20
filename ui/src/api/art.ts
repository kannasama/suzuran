import client from './client'

export const artApi = {
  embedFromUrl(trackId: number, sourceUrl: string) {
    return client.post(`/tracks/${trackId}/art/embed`, { source_url: sourceUrl })
  },
  extract(trackId: number) {
    return client.post(`/tracks/${trackId}/art/extract`)
  },
  standardize(trackId: number, artProfileId: number) {
    return client.post(`/tracks/${trackId}/art/standardize`, { art_profile_id: artProfileId })
  },
  standardizeLibrary(libraryId: number, artProfileId: number) {
    return client.post(`/libraries/${libraryId}/art/standardize`, { art_profile_id: artProfileId })
  },
}
