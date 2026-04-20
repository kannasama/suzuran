export interface ArtProfile {
  id: number;
  name: string;
  max_width_px: number;
  max_height_px: number;
  max_size_bytes?: number;
  format: 'jpeg' | 'png';
  quality: number;
  apply_to_library_id?: number;
  created_at: string;
}

export interface UpsertArtProfile {
  name: string;
  max_width_px: number;
  max_height_px: number;
  max_size_bytes?: number;
  format: 'jpeg' | 'png';
  quality: number;
  apply_to_library_id?: number;
}
