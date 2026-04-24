export interface Issue {
  id: number
  library_id: number
  track_id: number | null
  issue_type: 'missing_file' | 'bad_audio_properties' | 'untagged' | 'duplicate_mb_id'
  detail: string | null
  severity: 'high' | 'medium' | 'low'
  dismissed: boolean
  resolved: boolean
  created_at: string
  updated_at: string
}
