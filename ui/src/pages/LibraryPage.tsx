import { useState, useEffect, useRef, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'
import { TrackEditPanel } from '../components/TrackEditPanel'
import { AlternativesPanel } from '../components/AlternativesPanel'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { useAuth } from '../contexts/AuthContext'
import { getLibrary, listLibraryTracks } from '../api/libraries'
import { enqueueLookup } from '../api/tracks'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import client from '../api/client'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'

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

  const qc = useQueryClient()
  const [selectedLibraryId, setSelectedLibraryId] = useState<number | null>(null)
  const [selectedVirtualLibraryId, setSelectedVirtualLibraryId] = useState<number | null>(null)
  const [scanQueued, setScanQueued] = useState(false)
  const [visibleColumns, setVisibleColumns] = useState<Set<string>>(loadColumnVisibility)
  const [showColumnPicker, setShowColumnPicker] = useState(false)
  const pickerRef = useRef<HTMLDivElement>(null)
  const [expandedTrackId, setExpandedTrackId] = useState<number | null>(null)
  const [editingTagsTrackId, setEditingTagsTrackId] = useState<number | null>(null)
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)

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

  const { data: suggestions = [] } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
    enabled: selectedLibraryId != null,
  })

  const suggestionsByTrack = useMemo(() => {
    const map: Record<number, TagSuggestion> = {}
    for (const s of suggestions) {
      const ex = map[s.track_id]
      if (!ex || s.confidence > ex.confidence) map[s.track_id] = s
    }
    return map
  }, [suggestions])

  const acceptMutation = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.accept(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
      qc.invalidateQueries({ queryKey: ['library-tracks', selectedLibraryId] })
    },
  })

  const rejectMutation = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.reject(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tag-suggestions'] }),
  })

  const lookupMutation = useMutation({
    mutationFn: (trackId: number) => enqueueLookup(trackId),
  })

  async function handleScan() {
    if (selectedLibraryId == null) return
    try {
      await client.post('/jobs/scan', { library_id: selectedLibraryId })
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
                <TrackRow
                  key={track.id}
                  track={track}
                  visibleColumns={visibleColumns}
                  suggestion={suggestionsByTrack[track.id]}
                  isExpanded={expandedTrackId === track.id}
                  isEditingTags={editingTagsTrackId === track.id}
                  onToggleExpand={() => {
                    setExpandedTrackId(prev => prev === track.id ? null : track.id)
                    setEditingTagsTrackId(null)
                  }}
                  onSearch={() => setSearchTrack(track)}
                  onLookup={() => lookupMutation.mutate(track.id)}
                  onAccept={id => acceptMutation.mutate(id)}
                  onReject={id => rejectMutation.mutate(id)}
                  onEditTags={() => setEditingTagsTrackId(track.id)}
                  onEditTagsClose={() => setEditingTagsTrackId(null)}
                />
              ))
            )}
          </div>
        </main>
      </div>
      {searchTrack != null && (
        <IngestSearchDialog
          track={searchTrack}
          onClose={() => {
            setSearchTrack(null)
            qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
          }}
        />
      )}
    </div>
  )
}

function TrackRow({
  track,
  visibleColumns,
  suggestion,
  isExpanded,
  isEditingTags,
  onToggleExpand,
  onSearch,
  onLookup,
  onAccept,
  onReject,
  onEditTags,
  onEditTagsClose,
}: {
  track: Track
  visibleColumns: Set<string>
  suggestion?: TagSuggestion
  isExpanded: boolean
  isEditingTags: boolean
  onToggleExpand: () => void
  onSearch: () => void
  onLookup: () => void
  onAccept: (id: number) => void
  onReject: (id: number) => void
  onEditTags: () => void
  onEditTagsClose: () => void
}) {
  const pct = suggestion ? Math.round(suggestion.confidence * 100) : null
  const [showAlt, setShowAlt] = useState(false)

  return (
    <>
      <div className={`flex items-center gap-0 px-2 py-0.5 border-b border-border-subtle text-xs hover:bg-bg-row-hover ${isExpanded ? 'bg-bg-surface' : ''}`}>
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
          <span className="w-16 shrink-0 flex items-center gap-1 justify-end">
            {suggestion && (
              <span
                className={`text-[10px] font-mono ${pct! >= 80 ? 'text-green-400' : 'text-yellow-400'}`}
                title={`Pending suggestion (${pct}% confidence)`}
              >
                {pct}%
              </span>
            )}
            <button
              onClick={onToggleExpand}
              className={`text-xs border rounded px-1.5 py-0.5 transition-colors ${
                isExpanded
                  ? 'border-accent text-accent'
                  : 'border-border text-text-muted hover:border-accent hover:text-text-secondary'
              }`}
              title="Track actions"
            >
              ⋯
            </button>
          </span>
        )}
      </div>

      {isExpanded && (
        <div className="border-b border-border bg-bg-surface px-3 py-2 flex flex-col gap-2">
          {/* Action buttons */}
          <div className="flex items-center gap-1.5">
            <button
              onClick={onLookup}
              className="text-xs border border-border text-text-muted rounded px-2 py-0.5 hover:bg-bg-panel hover:border-accent hover:text-text-secondary"
            >
              Lookup
            </button>
            <button
              onClick={onSearch}
              className="text-xs border border-border text-text-muted rounded px-2 py-0.5 hover:bg-bg-panel hover:border-accent hover:text-text-secondary"
            >
              Search
            </button>
            <button
              onClick={isEditingTags ? onEditTagsClose : onEditTags}
              className={`text-xs border rounded px-2 py-0.5 ${
                isEditingTags
                  ? 'border-accent text-accent'
                  : 'border-border text-text-muted hover:bg-bg-panel hover:border-accent hover:text-text-secondary'
              }`}
            >
              Edit Tags
            </button>
            {suggestion?.alternatives && suggestion.alternatives.length > 0 && (
              <button
                onClick={() => setShowAlt(v => !v)}
                className={`text-xs border rounded px-2 py-0.5 ${
                  showAlt
                    ? 'border-accent text-accent'
                    : 'border-border text-text-muted hover:bg-bg-panel hover:border-accent hover:text-text-secondary'
                }`}
              >
                Alt…
              </button>
            )}
          </div>

          {/* Pending suggestion */}
          {suggestion && !isEditingTags && (
            <div className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <span className="text-[10px] uppercase tracking-wide text-text-muted font-mono">{suggestion.source}</span>
                <span className={`text-[10px] font-mono ${pct! >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>{pct}% confidence</span>
                <button
                  onClick={() => onAccept(suggestion.id)}
                  className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 hover:opacity-90 ml-auto"
                >
                  Accept
                </button>
                <button
                  onClick={() => onReject(suggestion.id)}
                  className="text-xs border border-border text-text-muted rounded px-2 py-0.5 hover:bg-bg-panel"
                >
                  Reject
                </button>
              </div>
              <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[11px]">
                {Object.entries(suggestion.suggested_tags).slice(0, 8).map(([k, v]) => (
                  <div key={k} className="flex gap-1.5 min-w-0">
                    <span className="text-text-muted font-mono w-28 shrink-0 truncate">{k}</span>
                    <span className="text-text-secondary truncate">{v}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Alternatives picker */}
          {showAlt && suggestion && !isEditingTags && (
            <AlternativesPanel suggestion={suggestion} onClose={() => setShowAlt(false)} />
          )}

          {/* Tag editor */}
          {isEditingTags && (
            <TrackEditPanel track={track} suggestion={suggestion} onClose={onEditTagsClose} />
          )}
        </div>
      )}
    </>
  )
}
