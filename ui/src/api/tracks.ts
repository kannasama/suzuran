import client from './client';
import type { Track } from '../types/track';

export const tracksApi = {
  getTrack(id: number): Promise<Track> {
    return client.get<Track>(`/tracks/${id}`).then(r => r.data);
  },
};

export function enqueueLookup(id: number): Promise<void> {
  return client.post(`/tracks/${id}/lookup`).then(() => {});
}

export interface ScheduleDeleteResult {
  job_id: number
  run_after: string
}

export function scheduleDelete(ids: number[], immediate = false): Promise<ScheduleDeleteResult> {
  return client.post<ScheduleDeleteResult>('/tracks/delete', { ids, immediate }).then(r => r.data);
}

export function getPendingTags(id: number): Promise<Record<string, string>> {
  return client.get<{ tags: Record<string, string> }>(`/tracks/${id}/pending-tags`)
    .then(r => r.data.tags ?? {});
}

export function setPendingTags(id: number, tags: Record<string, string>): Promise<void> {
  return client.put(`/tracks/${id}/pending-tags`, { tags }).then(() => {});
}

export function clearPendingTags(id: number): Promise<void> {
  return client.delete(`/tracks/${id}/pending-tags`).then(() => {});
}

export function applyTags(id: number): Promise<void> {
  return client.post(`/tracks/${id}/apply-tags`).then(() => {});
}
