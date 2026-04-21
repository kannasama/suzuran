export interface LibraryProfile {
  id: number;
  library_id: number;
  encoding_profile_id: number;
  derived_dir_name: string;
  include_on_submit: boolean;
  auto_include_above_hz: number | null;
  created_at: string;
}

export interface UpsertLibraryProfile {
  library_id: number;
  encoding_profile_id: number;
  derived_dir_name: string;
  include_on_submit: boolean;
  auto_include_above_hz: number | null;
}
