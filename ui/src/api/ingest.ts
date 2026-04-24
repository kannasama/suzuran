import client from './client'
import type { Track } from '../types/track'

interface ProcessStagedPayload {
  track_id: number;
  tag_suggestion_id?: number;
  cover_art_url?: string;
  write_folder_art: boolean;
  profile_ids: number[];
  supersede_track_id?: number;
  supersede_profile_id?: number | null;
}

export interface ProfileMatchInfo {
  library_profile_id: number;
  profile_name: string;
  derived_dir_name: string;
}

export interface SupersedeMatchInfo {
  active_track_id: number;
  active_track_format: string;
  active_track_sample_rate: number | null;
  active_track_bit_depth: number | null;
  active_track_bitrate: number | null;
  active_quality_rank: number;
  staged_quality_rank: number;
  identity_method: 'mb_recording_id' | 'tag_tuple' | 'acoustid';
  is_upgrade: boolean;
  profile_match: ProfileMatchInfo | null;
}

export interface SupersedeCheckResult {
  track_id: number;
  match: SupersedeMatchInfo | null;
}

export function getStagedTracks(): Promise<Track[]> {
  return client.get<Track[]>('/ingest/staged').then(r => r.data)
}

export function getStagedCount(): Promise<number> {
  return client.get<{ count: number }>('/ingest/count').then(r => r.data.count)
}

export function submitTrack(payload: ProcessStagedPayload): Promise<{ job_id: number }> {
  return client.post<{ job_id: number }>('/ingest/submit', payload).then(r => r.data)
}

export function checkSupersede(trackIds: number[]): Promise<SupersedeCheckResult[]> {
  return client
    .post<SupersedeCheckResult[]>('/ingest/supersede-check', { track_ids: trackIds })
    .then(r => r.data)
}
