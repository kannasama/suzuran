import client from './client'
import type { Track } from '../types/track'

interface ProcessStagedPayload {
  track_id: number;
  tag_suggestion_id?: number;
  cover_art_url?: string;
  write_folder_art: boolean;
  profile_ids: number[];
}

export function getStagedTracks(): Promise<Track[]> {
  return client.get<Track[]>('/ingest/staged').then(r => r.data)
}

export function submitTrack(payload: ProcessStagedPayload): Promise<{ job_id: number }> {
  return client.post<{ job_id: number }>('/ingest/submit', payload).then(r => r.data)
}
