import { useState, useEffect, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'
import { useAuth } from '../contexts/AuthContext'
import { getLibrary, listLibraryTracks } from '../api/libraries'
import client from '../api/client'
import type { Track } from '../types/track'

interface ColumnDef {
  key: string
  label: string        // shown in the picker
  headerLabel?: string // shown in the column header (falls back to label)
  className: string
}

const COLUMNS: ColumnDef[] = [
  { key: 'num',      label: 'Track #',  headerLabel: '#',    className: 'w-6' },
  { key: 'title',    label: 'Title',                         className: 'flex-[3]' },
  { key: 'artist',   label: 'Artist',                        className: 'flex-[2]' },
  { key: 'album',    label: 'Album',                         className: 'flex-[2]' },
  { key: 'year',     label: 'Year',                          className: 'w-10' },
  { key: 'genre',    label: 'Genre',                         className: 'flex-1' },
  { key: 'format',   label: 'Format',                        className: 'w-12' },
  { key: 'bitrate',  label: 'Bitrate',                       className: 'w-14' },
  { key: 'duration', label: 'Duration', headerLabel: 'Time', className: 'w-10' },
  { key: 'actions',  label: 'Actions',                       className: 'w-16' },
]

const LS_KEY = 'suzuran:column-visibility'

function loadColumnVisibility(): Set<string> {
  try {
    const raw = localStorage.getItem(LS_KEY)
    if (raw) {
      const arr = JSON.parse(raw)
      if (Array.isArray(arr)) return new Set(arr as string[])
    }
  } catch { /* ignore */ }
  return new Set(COLUMNS.map(c => c.key))
}

function formatDuration(secs?: number): string {
  if (secs == null) return '—'
  const m = Math.floor(secs / 60)
  const s = Math.floor(secs % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

function formatBitrate(bps?: number): string {
  if (bps == null) return '—'
  return `${Math.round(bps / 1000)}k`
}

function getFileExtension(path: string): string {
  const dot = path.lastIndexOf('.')
  if (dot === -1) return '—'
  return path.slice(dot + 1).toLowerCase()
}

export function LibraryPage() {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const isLibraryAdmin = user?.role === 'admin' || user?.role === 'library_admin'

  const [selectedLibraryId, setSelectedLibraryId] = useState<number | null>(null)
  const [selectedVirtualLibraryId, setSelectedVirtualLibraryId] = useState<number | null>(null)
  const [scanQueued, setScanQueued] = useState(false)
  const [visibleColumns, setVisibleColumns] = useState<Set<string>>(loadColumnVisibility)
  const [showColumnPicker, setShowColumnPicker] = useState(false)
  const pickerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!showColumnPicker) return
    function handleMouseDown(e: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowColumnPicker(false)
      }
    }
    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [showColumnPicker])

  function toggleColumn(key: string) {
    setVisibleColumns(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      localStorage.setItem(LS_KEY, JSON.stringify([...next]))
      return next
    })
  }

  const { data: selectedLibrary } = useQuery({
    queryKey: ['library', selectedLibraryId],
    queryFn: () => getLibrary(selectedLibraryId!),
    enabled: selectedLibraryId != null,
  })

  const { data: tracks = [], isLoading: tracksLoading } = useQuery({
    queryKey: ['library-tracks', selectedLibraryId],
    queryFn: () => listLibraryTracks(selectedLibraryId!),
    enabled: selectedLibraryId != null,
  })

  async function handleScan() {
    if (selectedLibraryId == null) return
    try {
      await client.post('/jobs', { job_type: 'scan', payload: { library_id: selectedLibraryId } })
      setScanQueued(true)
      setTimeout(() => setScanQueued(false), 2000)
    } catch {
      // ignore — job queue errors are not critical UI failures
    }
  }

  function getToolbarLabel() {
    if (selectedLibraryId == null && selectedVirtualLibraryId == null) {
      return 'Select a library'
    }
    if (selectedVirtualLibraryId != null) {
      return `Virtual Library #${selectedVirtualLibraryId}`
    }
    return selectedLibrary?.name ?? `Library #${selectedLibraryId}`
  }

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-1 overflow-hidden">
        {/* Left: tree pane */}
        <aside className="w-44 flex-shrink-0 bg-bg-panel border-r border-border overflow-y-auto">
          <LibraryTree
            isAdmin={isAdmin}
            isLibraryAdmin={isLibraryAdmin}
            selectedLibraryId={selectedLibraryId}
            onSelectLibrary={id => { setSelectedLibraryId(id); setSelectedVirtualLibraryId(null) }}
            selectedVirtualLibraryId={selectedVirtualLibraryId}
            onSelectVirtualLibrary={id => { setSelectedVirtualLibraryId(id); setSelectedLibraryId(null) }}
          />
        </aside>

        {/* Right: track list */}
        <main className="flex flex-col flex-1 overflow-hidden">
          {/* Toolbar */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
            <span className="text-text-muted text-xs">
              {getToolbarLabel()}
            </span>
            <div className="ml-auto flex gap-1 items-center">
              {selectedLibraryId != null && selectedVirtualLibraryId == null && (
                <>
                  {scanQueued && (
                    <span className="text-xs text-accent mr-1">Scan queued</span>
                  )}
                  <button
                    onClick={handleScan}
                    className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:text-text-primary hover:border-accent"
                    title="Scan this library for new/changed files"
                  >
                    Scan
                  </button>
                </>
              )}
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Group: None ▾
              </button>
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Sort ▾
              </button>
            </div>
          </div>

          {/* Column headers */}
          <div className="flex items-center gap-0 px-2 py-1 bg-bg-panel border-b border-border text-text-muted text-[11px] uppercase tracking-wider flex-shrink-0">
            <span className="w-5 shrink-0"></span>
            {COLUMNS.map(col => visibleColumns.has(col.key) && (
              <span key={col.key} className={col.className + ' shrink-0'}>
                {col.headerLabel ?? col.label}
              </span>
            ))}
            {/* Column picker */}
            <div ref={pickerRef} className="relative w-6 shrink-0 ml-auto">
              <span
                className="text-accent cursor-pointer hover:opacity-70 transition-opacity block text-center"
                onClick={() => setShowColumnPicker(v => !v)}
                title="Customize columns"
              >
                ⊕
              </span>
              {showColumnPicker && (
                <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[140px]">
                  {COLUMNS.map(col => (
                    <label
                      key={col.key}
                      className="flex items-center gap-2 px-3 py-1 hover:bg-bg-row-hover cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={visibleColumns.has(col.key)}
                        onChange={() => toggleColumn(col.key)}
                        className="accent-[color:var(--accent)]"
                      />
                      <span className="text-text-primary text-xs normal-case tracking-normal">
                        {col.label}
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Track list area */}
          <div className="flex-1 overflow-y-auto">
            {selectedLibraryId == null && selectedVirtualLibraryId == null ? (
              <div className="flex items-center justify-center h-32 text-text-muted text-xs">
                Select a library from the tree to view tracks.
              </div>
            ) : tracksLoading ? (
              <div className="flex items-center justify-center h-32 text-text-muted text-xs">
                Loading tracks…
              </div>
            ) : tracks.length === 0 ? (
              <div className="flex items-center justify-center h-32 text-text-muted text-xs">
                No tracks in this library. Run a scan to discover files.
              </div>
            ) : (
              tracks.map((track: Track) => (
                <TrackRow key={track.id} track={track} visibleColumns={visibleColumns} />
              ))
            )}
          </div>
        </main>
      </div>
    </div>
  )
}

function TrackRow({ track, visibleColumns }: { track: Track; visibleColumns: Set<string> }) {
  return (
    <div className="flex items-center gap-0 px-2 py-0.5 border-b border-border-subtle text-xs hover:bg-bg-row-hover">
      <span className="w-5 shrink-0 text-text-muted text-[10px]"></span>
      {visibleColumns.has('num') && (
        <span className="w-6 shrink-0 text-text-muted font-mono">{track.tracknumber ?? '—'}</span>
      )}
      {visibleColumns.has('title') && (
        <span className="flex-[3] shrink-0 text-text-primary truncate pr-2">{track.title ?? '—'}</span>
      )}
      {visibleColumns.has('artist') && (
        <span className="flex-[2] shrink-0 text-text-secondary truncate pr-2">{track.artist ?? '—'}</span>
      )}
      {visibleColumns.has('album') && (
        <span className="flex-[2] shrink-0 text-text-secondary truncate pr-2">{track.album ?? '—'}</span>
      )}
      {visibleColumns.has('year') && (
        <span className="w-10 shrink-0 text-text-muted">{track.date?.slice(0, 4) ?? '—'}</span>
      )}
      {visibleColumns.has('genre') && (
        <span className="flex-1 shrink-0 text-text-muted truncate pr-2">{track.genre ?? '—'}</span>
      )}
      {visibleColumns.has('format') && (
        <span className="w-12 shrink-0 text-text-muted font-mono uppercase text-[10px]">
          {getFileExtension(track.relative_path)}
        </span>
      )}
      {visibleColumns.has('bitrate') && (
        <span className="w-14 shrink-0 text-text-muted font-mono">{formatBitrate(track.bitrate)}</span>
      )}
      {visibleColumns.has('duration') && (
        <span className="w-10 shrink-0 text-text-muted font-mono">{formatDuration(track.duration_secs)}</span>
      )}
      {visibleColumns.has('actions') && (
        <span className="w-16 shrink-0"></span>
      )}
    </div>
  )
}
