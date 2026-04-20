export interface TagSuggestion {
  id: number;
  track_id: number;
  source: 'acoustid' | 'mb_search' | 'freedb';
  suggested_tags: Record<string, string>;
  confidence: number;
  mb_recording_id?: string;
  mb_release_id?: string;
  cover_art_url?: string;
  status: 'pending' | 'accepted' | 'rejected';
  created_at: string;
}
