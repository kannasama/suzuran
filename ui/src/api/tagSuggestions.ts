import client from './client';
import type { TagSuggestion } from '../types/tagSuggestion';

interface CreateTagSuggestionBody {
  track_id: number;
  source: string;
  suggested_tags: Record<string, string>;
  confidence: number;
  cover_art_url?: string;
  musicbrainz_recording_id?: string;
  musicbrainz_release_id?: string;
}

export const tagSuggestionsApi = {
  listPending(trackId?: number) {
    return client
      .get<TagSuggestion[]>('/tag-suggestions', {
        params: trackId != null ? { track_id: trackId } : {},
      })
      .then(r => r.data);
  },

  count(): Promise<number> {
    return client
      .get<{ count: number }>('/tag-suggestions/count')
      .then(r => r.data.count);
  },

  accept(id: number, fields?: string[]) {
    return client.post(
      `/tag-suggestions/${id}/accept`,
      fields ? { fields } : undefined,
    );
  },

  reject(id: number) {
    return client.post(`/tag-suggestions/${id}/reject`);
  },

  batchAccept(minConfidence: number) {
    return client
      .post<{ accepted: number }>('/tag-suggestions/batch-accept', {
        min_confidence: minConfidence,
      })
      .then(r => r.data);
  },

  create(body: CreateTagSuggestionBody): Promise<TagSuggestion> {
    return client.post<TagSuggestion>('/tag-suggestions', body).then(r => r.data);
  },
};
