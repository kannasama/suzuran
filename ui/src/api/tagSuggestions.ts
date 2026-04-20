import client from './client';
import type { TagSuggestion } from '../types/tagSuggestion';

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

  accept(id: number) {
    return client.post(`/tag-suggestions/${id}/accept`);
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
};
