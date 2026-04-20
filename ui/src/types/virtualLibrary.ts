export interface VirtualLibrary {
  id: number;
  name: string;
  root_path: string;
  link_type: 'symlink' | 'hardlink';
  created_at: string;
}

export interface VirtualLibrarySource {
  virtual_library_id: number;
  library_id: number;
  priority: number;
}

export interface UpsertVirtualLibrary {
  name: string;
  root_path: string;
  link_type: 'symlink' | 'hardlink';
}
