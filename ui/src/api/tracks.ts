import client from './client';
import type { Track } from '../types/track';

export const tracksApi = {
  getTrack(id: number): Promise<Track> {
    return client.get<Track>(`/tracks/${id}`).then(r => r.data);
  },
};
