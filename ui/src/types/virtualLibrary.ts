export interface VirtualLibrary {
  id: number;
  name: string;
  root_path: string;
  link_type: 'symlink' | 'hardlink';
  created_at: string;
}

export interface VirtualLibrarySource {
  id: number;
  virtual_library_id: number;
  library_id: number;
  library_profile_id: number | null;
  priority: number;
}

export interface UpsertVirtualLibrary {
  name: string;
  root_path: string;
  link_type: 'symlink' | 'hardlink';
}
