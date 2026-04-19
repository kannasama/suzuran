import { useState } from 'react'
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'
import { useAuth } from '../contexts/AuthContext'

export function LibraryPage() {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'

  const [selectedLibraryId, setSelectedLibraryId] = useState<number | null>(null)

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-1 overflow-hidden">
        {/* Left: tree pane */}
        <aside className="w-44 flex-shrink-0 bg-bg-panel border-r border-border overflow-y-auto">
          <LibraryTree
            isAdmin={isAdmin}
            selectedLibraryId={selectedLibraryId}
            onSelectLibrary={setSelectedLibraryId}
          />
        </aside>

        {/* Right: track list */}
        <main className="flex flex-col flex-1 overflow-hidden">
          {/* Toolbar */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
            <span className="text-text-muted text-xs">
              {selectedLibraryId == null ? 'Select a library' : `Library #${selectedLibraryId}`}
            </span>
            <div className="ml-auto flex gap-1">
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Group: None ▾
              </button>
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Sort ▾
              </button>
            </div>
          </div>

          {/* Column headers */}
          <div className="flex items-center gap-0 px-2 py-1 bg-bg-panel border-b border-border text-text-muted text-[9px] uppercase tracking-wider flex-shrink-0">
            <span className="w-5"></span>
            <span className="w-6">#</span>
            <span className="flex-[3]">Title</span>
            <span className="flex-[2]">Artist</span>
            <span className="flex-[2]">Album</span>
            <span className="w-10">Year</span>
            <span className="flex-1">Genre</span>
            <span className="w-12">Format</span>
            <span className="w-14">Bitrate</span>
            <span className="w-10">Time</span>
            <span className="w-6 text-accent cursor-pointer" title="Customize columns">⊕</span>
          </div>

          {/* Track list area (stub — populated in a future subphase) */}
          <div className="flex-1 overflow-y-auto">
            <div className="flex items-center justify-center h-32 text-text-muted text-xs">
              {selectedLibraryId == null
                ? 'Select a library from the tree to view tracks.'
                : 'Track list coming in a future subphase.'}
            </div>
          </div>
        </main>
      </div>
    </div>
  )
}
