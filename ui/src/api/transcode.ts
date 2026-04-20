import client from './client'

export const transcodeApi = {
  transcodeTrack(trackId: number, targetLibraryId: number) {
    return client.post(`/tracks/${trackId}/transcode`, { target_library_id: targetLibraryId })
  },
  transcodeLibrary(srcLibId: number, targetLibraryId: number) {
    return client.post(`/libraries/${srcLibId}/transcode`, { target_library_id: targetLibraryId })
  },
  transcodeSync(srcLibId: number, targetLibraryId: number) {
    return client.post(`/libraries/${srcLibId}/transcode-sync`, { target_library_id: targetLibraryId })
  },
}
