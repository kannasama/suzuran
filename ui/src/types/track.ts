export interface Track {
  id: number;
  library_id: number;
  relative_path: string;
  file_hash: string;
  title?: string;
  artist?: string;
  albumartist?: string;
  album?: string;
  tracknumber?: string;
  discnumber?: string;
  totaldiscs?: string;
  totaltracks?: string;
  date?: string;
  genre?: string;
  composer?: string;
  label?: string;
  catalognumber?: string;
  tags: Record<string, unknown>;
  duration_secs?: number;
  bitrate?: number;
  sample_rate?: number;
  channels?: number;
  has_embedded_art: boolean;
  acoustid_fingerprint?: string;
  last_scanned_at: string;
  created_at: string;
}
