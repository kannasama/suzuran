export interface Track {
  id: number;
  library_id: number;
  relative_path: string;
  title?: string;
  artist?: string;
  albumartist?: string;
  album?: string;
  tracknumber?: string;
  date?: string;
  genre?: string;
  tags: Record<string, unknown>;
}
