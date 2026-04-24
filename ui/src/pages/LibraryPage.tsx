import React, { useState, useEffect, useRef, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'
import { AlternativesPanel } from '../components/AlternativesPanel'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { useAuth } from '../contexts/AuthContext'
import { getLibrary, listLibraryTracks } from '../api/libraries'
import { enqueueLookup } from '../api/tracks'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import client from '../api/client'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'

// ── Tag field definitions ──────────────────────────────────────────────────────
interface TagField { key: string; label: string; cols?: number }

const COL_SPAN: Record<number, string> = {
  1: 'col-span-1',
  2: 'col-span-2',
  3: 'col-span-3',
  4: 'col-span-4',
  6: 'col-span-6',
}

const BULK_EDIT_FIELDS: TagField[] = [
  { key: 'title',                      label: 'Title',                 cols: 4 },
  { key: 'date',                       label: 'Date',                  cols: 2 },
  { key: 'artist',                     label: 'Artist',                cols: 3 },
  { key: 'albumartist',                label: 'Album Artist',          cols: 3 },
  { key: 'album',                      label: 'Album',                 cols: 4 },
  { key: 'genre',                      label: 'Genre',                 cols: 2 },
  { key: 'tracknumber',                label: 'Track #',               cols: 1 },
  { key: 'totaltracks',                label: 'Total Tracks',          cols: 1 },
  { key: 'discnumber',                 label: 'Disc #',                cols: 1 },
  { key: 'totaldiscs',                 label: 'Total Discs',           cols: 1 },
  { key: 'releasecountry',             label: 'Release Country',       cols: 1 },
  { key: 'originalyear',               label: 'Original Year',         cols: 1 },
  { key: 'albumartistsort',            label: 'Album Artist Sort',     cols: 3 },
  { key: 'artistsort',                 label: 'Artist Sort',           cols: 3 },
  { key: 'releasetype',                label: 'Release Type',          cols: 2 },
  { key: 'releasestatus',              label: 'Release Status',        cols: 2 },
  { key: 'originaldate',               label: 'Original Release Date', cols: 2 },
  { key: 'label',                      label: 'Record Label',          cols: 3 },
  { key: 'catalognumber',              label: 'Catalog #',             cols: 2 },
  { key: 'barcode',                    label: 'Barcode',               cols: 1 },
  { key: 'musicbrainz_artistid',       label: 'MB Artist ID',          cols: 6 },
  { key: 'musicbrainz_albumartistid',  label: 'MB Release Artist ID',  cols: 6 },
  { key: 'musicbrainz_releasegroupid', label: 'MB Release Group ID',   cols: 6 },
  { key: 'musicbrainz_releaseid',      label: 'MB Release ID',         cols: 6 },
  { key: 'musicbrainz_trackid',        label: 'MB Recording ID',       cols: 6 },
]

// Fields promoted to top-level on Track; rest come from track.tags
const TOP_LEVEL_TAG_FIELDS = new Set([
  'title', 'artist', 'albumartist', 'album', 'tracknumber', 'discnumber',
  'totaldiscs', 'totaltracks', 'date', 'genre', 'label', 'catalognumber',
])

function getTrackTagValue(track: Track, key: string): string {
  if (TOP_LEVEL_TAG_FIELDS.has(key)) {
    return (track as unknown as Record<string, string | undefined>)[key] ?? ''
  }
  const v = track.tags[key]
  if (typeof v === 'string') return v
  if (v != null) return String(v)
  return ''
}

// ── Column definitions ─────────────────────────────────────────────────────────
interface ColumnDef { key: string; label: string; headerLabel?: string; className: string }

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

// ── Sort / group types ─────────────────────────────────────────────────────────
type GroupByKey = 'none' | 'album' | 'artist' | 'albumartist' | 'year' | 'genre'
type SortByKey = 'tracknumber' | 'discnumber' | 'title' | 'artist' | 'album' | 'year' | 'duration' | 'bitrate'
type SortLevel = { key: SortByKey; dir: 'asc' | 'desc' }
type MenuItem = { label: string; action: () => void } | null

const GROUP_OPTIONS: { key: GroupByKey; label: string }[] = [
  { key: 'none',        label: 'None' },
  { key: 'album',       label: 'Album' },
  { key: 'artist',      label: 'Artist' },
  { key: 'albumartist', label: 'Album Artist' },
  { key: 'year',        label: 'Year' },
  { key: 'genre',       label: 'Genre' },
]

const SORT_OPTIONS: { key: SortByKey; label: string }[] = [
  { key: 'tracknumber', label: 'Track #' },
  { key: 'discnumber',  label: 'Disc #' },
  { key: 'title',       label: 'Title' },
  { key: 'artist',      label: 'Artist' },
  { key: 'album',       label: 'Album' },
  { key: 'year',        label: 'Year' },
  { key: 'duration',    label: 'Duration' },
  { key: 'bitrate',     label: 'Bitrate' },
]

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
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<number>>(new Set())
  const lastSelectedIdRef = useRef<number | null>(null)
  const [groupBy, setGroupBy] = useState<GroupByKey>('none')
  const [sortLevels, setSortLevels] = useState<SortLevel[]>([{ key: 'tracknumber', dir: 'asc' }])
  const [showGroupMenu, setShowGroupMenu] = useState(false)
  const [showSortMenu, setShowSortMenu] = useState(false)
  const groupMenuRef = useRef<HTMLDivElement>(null)
  const sortMenuRef = useRef<HTMLDivElement>(null)
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null)
  const [altPanelTrackId, setAltPanelTrackId] = useState<number | null>(null)

  // ── Dismiss handlers ──────────────────────────────────────────────────────────
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

  useEffect(() => {
    if (!showGroupMenu && !showSortMenu) return
    function handleMouseDown(e: MouseEvent) {
      if (showGroupMenu && groupMenuRef.current && !groupMenuRef.current.contains(e.target as Node)) {
        setShowGroupMenu(false)
      }
      if (showSortMenu && sortMenuRef.current && !sortMenuRef.current.contains(e.target as Node)) {
        setShowSortMenu(false)
      }
    }
    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [showGroupMenu, showSortMenu])

  useEffect(() => {
    if (!contextMenu) return
    function handleClick() { setContextMenu(null) }
    function handleScroll() { setContextMenu(null) }
    document.addEventListener('click', handleClick)
    document.addEventListener('scroll', handleScroll, true)
    return () => {
      document.removeEventListener('click', handleClick)
      document.removeEventListener('scroll', handleScroll, true)
    }
  }, [contextMenu])

  function toggleColumn(key: string) {
    setVisibleColumns(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      localStorage.setItem(LS_KEY, JSON.stringify([...next]))
      return next
    })
  }

  // ── Queries ───────────────────────────────────────────────────────────────────
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

  // ── Display groups + flat/selected track lists ────────────────────────────────
  const displayGroups = useMemo(() => {
    function getTrackSortVal(t: Track, key: SortByKey): string | number {
      switch (key) {
        case 'tracknumber': {
          const n = parseInt((t.tracknumber ?? '').split('/')[0], 10)
          return isNaN(n) ? 9999 : n
        }
        case 'discnumber': {
          const n = parseInt((t.discnumber ?? '').split('/')[0], 10)
          return isNaN(n) ? 9999 : n
        }
        case 'title':    return (t.title ?? '').toLowerCase()
        case 'artist':   return (t.artist ?? '').toLowerCase()
        case 'album':    return (t.album ?? '').toLowerCase()
        case 'year':     return t.date?.slice(0, 4) ?? ''
        case 'duration': return t.duration_secs ?? 0
        case 'bitrate':  return t.bitrate ?? 0
      }
    }
    function cmp(a: string | number, b: string | number): number {
      if (typeof a === 'number' && typeof b === 'number') return a - b
      return String(a).localeCompare(String(b))
    }
    function sortTracks(arr: Track[]): Track[] {
      if (sortLevels.length === 0) return arr
      return [...arr].sort((a, b) => {
        for (const { key, dir } of sortLevels) {
          const c = cmp(getTrackSortVal(a, key), getTrackSortVal(b, key))
          if (c !== 0) return dir === 'asc' ? c : -c
        }
        return 0
      })
    }
    function getGroupKey(t: Track): string {
      switch (groupBy) {
        case 'none':        return ''
        case 'album':       return `${t.albumartist ?? t.artist ?? '—'} — ${t.album ?? 'Unknown Album'}`
        case 'artist':      return t.artist ?? '—'
        case 'albumartist': return t.albumartist ?? '—'
        case 'year':        return t.date?.slice(0, 4) ?? '—'
        case 'genre':       return t.genre ?? '—'
      }
    }
    if (groupBy === 'none') {
      return [{ key: '', tracks: sortTracks(tracks) }]
    }
    const groupMap = new Map<string, Track[]>()
    for (const t of tracks) {
      const k = getGroupKey(t)
      if (!groupMap.has(k)) groupMap.set(k, [])
      groupMap.get(k)!.push(t)
    }
    return [...groupMap.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, gt]) => ({ key, tracks: sortTracks(gt) }))
  }, [tracks, groupBy, sortLevels])

  const flatTracks = useMemo(() => displayGroups.flatMap(g => g.tracks), [displayGroups])
  const selectedTracks = useMemo(() => tracks.filter((t: Track) => selectedTrackIds.has(t.id)), [tracks, selectedTrackIds])

  // ── Sort label ────────────────────────────────────────────────────────────────
  function getSortLabel(): string {
    if (sortLevels.length === 0) return 'Sort'
    if (sortLevels.length === 1) {
      const opt = SORT_OPTIONS.find(o => o.key === sortLevels[0].key)
      return `Sort: ${opt?.label ?? sortLevels[0].key} ${sortLevels[0].dir === 'asc' ? '▲' : '▼'}`
    }
    return `Sort (${sortLevels.length})`
  }

  // ── Selection handlers ────────────────────────────────────────────────────────
  function handleRowClick(id: number, e: React.MouseEvent) {
    if (e.shiftKey && lastSelectedIdRef.current != null) {
      const ids = flatTracks.map(t => t.id)
      const a = ids.indexOf(lastSelectedIdRef.current)
      const b = ids.indexOf(id)
      if (a !== -1 && b !== -1) {
        const [lo, hi] = [Math.min(a, b), Math.max(a, b)]
        setSelectedTrackIds(prev => {
          const next = new Set(prev)
          ids.slice(lo, hi + 1).forEach(rid => next.add(rid))
          return next
        })
        return
      }
    }
    if (e.ctrlKey || e.metaKey) {
      setSelectedTrackIds(prev => {
        const next = new Set(prev)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        return next
      })
    } else {
      setSelectedTrackIds(new Set([id]))
    }
    lastSelectedIdRef.current = id
  }

  function handleCheckboxChange(id: number) {
    setSelectedTrackIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
    lastSelectedIdRef.current = id
  }

  function toggleSelectAll() {
    if (selectedTrackIds.size === tracks.length && tracks.length > 0) {
      setSelectedTrackIds(new Set())
    } else {
      setSelectedTrackIds(new Set(tracks.map((t: Track) => t.id)))
    }
  }

  // ── Context menu builders ─────────────────────────────────────────────────────
  function handleContextMenu(e: React.MouseEvent, track: Track) {
    e.preventDefault()
    e.stopPropagation()
    if (!selectedTrackIds.has(track.id)) {
      setSelectedTrackIds(new Set([track.id]))
      lastSelectedIdRef.current = track.id
    }
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: 'Lookup', action: () => { lookupMutation.mutate(track.id); setContextMenu(null) } },
        { label: 'Search', action: () => { setSearchTrack(track); setContextMenu(null) } },
        null,
        { label: 'Select All',   action: () => { toggleSelectAll(); setContextMenu(null) } },
        { label: 'Deselect All', action: () => { setSelectedTrackIds(new Set()); setContextMenu(null) } },
        null,
        { label: 'Copy Path', action: () => { navigator.clipboard.writeText(track.relative_path); setContextMenu(null) } },
      ],
    })
  }

  function handleThreeDotsClick(track: Track, x: number, y: number) {
    const suggestion = suggestionsByTrack[track.id]
    const items: MenuItem[] = [
      { label: 'Lookup', action: () => { lookupMutation.mutate(track.id); setContextMenu(null) } },
      { label: 'Search', action: () => { setSearchTrack(track); setContextMenu(null) } },
    ]
    if (suggestion) {
      const pct = Math.round(suggestion.confidence * 100)
      items.push(null)
      items.push({ label: `Accept (${pct}%)`, action: () => { acceptMutation.mutate(suggestion.id); setContextMenu(null) } })
      items.push({ label: 'Reject',           action: () => { rejectMutation.mutate(suggestion.id); setContextMenu(null) } })
    }
    if (suggestion?.alternatives && suggestion.alternatives.length > 0) {
      items.push(null)
      items.push({
        label: 'Alternatives…',
        action: () => {
          setAltPanelTrackId(prev => prev === track.id ? null : track.id)
          setContextMenu(null)
        },
      })
    }
    setContextMenu({ x, y, items })
  }

  // ── Mutations ─────────────────────────────────────────────────────────────────
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
    } catch { /* ignore */ }
  }

  function getToolbarLabel() {
    if (selectedLibraryId == null && selectedVirtualLibraryId == null) return 'Select a library'
    if (selectedVirtualLibraryId != null) return `Virtual Library #${selectedVirtualLibraryId}`
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
            onSelectLibrary={id => { setSelectedLibraryId(id); setSelectedVirtualLibraryId(null); setAltPanelTrackId(null) }}
            selectedVirtualLibraryId={selectedVirtualLibraryId}
            onSelectVirtualLibrary={id => { setSelectedVirtualLibraryId(id); setSelectedLibraryId(null); setAltPanelTrackId(null) }}
          />
        </aside>

        {/* Right: track list */}
        <main className="flex flex-col flex-1 overflow-hidden">
          {/* Toolbar */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
            <span className="text-text-muted text-xs">{getToolbarLabel()}</span>
            <div className="ml-auto flex gap-1 items-center">
              {selectedLibraryId != null && selectedVirtualLibraryId == null && (
                <>
                  {scanQueued && <span className="text-xs text-accent mr-1">Scan queued</span>}
                  <button
                    onClick={handleScan}
                    className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:text-text-primary hover:border-accent"
                    title="Scan this library for new/changed files"
                  >
                    Scan
                  </button>
                </>
              )}
              {/* Group by */}
              <div ref={groupMenuRef} className="relative">
                <button
                  onClick={() => { setShowGroupMenu(v => !v); setShowSortMenu(false) }}
                  className={`text-xs bg-bg-panel border rounded px-2 py-0.5 ${showGroupMenu ? 'border-accent text-accent' : 'border-border text-text-muted hover:text-text-primary'}`}
                >
                  Group: {GROUP_OPTIONS.find(o => o.key === groupBy)?.label ?? 'None'} ▾
                </button>
                {showGroupMenu && (
                  <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[130px]">
                    {GROUP_OPTIONS.map(opt => (
                      <button
                        key={opt.key}
                        onClick={() => { setGroupBy(opt.key); setShowGroupMenu(false) }}
                        className={`block w-full text-left px-3 py-1 text-xs hover:bg-bg-row-hover ${groupBy === opt.key ? 'text-accent' : 'text-text-primary'}`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              {/* Sort by — multi-level */}
              <div ref={sortMenuRef} className="relative">
                <button
                  onClick={() => { setShowSortMenu(v => !v); setShowGroupMenu(false) }}
                  className={`text-xs bg-bg-panel border rounded px-2 py-0.5 ${showSortMenu ? 'border-accent text-accent' : 'border-border text-text-muted hover:text-text-primary'}`}
                >
                  {getSortLabel()} ▾
                </button>
                {showSortMenu && (
                  <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-2 min-w-[230px]">
                    {sortLevels.length === 0 && (
                      <div className="px-3 py-1 text-xs text-text-muted italic">No sort applied</div>
                    )}
                    {sortLevels.map((level, i) => (
                      <div key={i} className="flex items-center gap-1 px-2 py-0.5">
                        <span className="text-[10px] text-text-muted w-3 shrink-0 text-center">{i + 1}</span>
                        <select
                          value={level.key}
                          onChange={e => setSortLevels(prev => prev.map((l, li) =>
                            li === i ? { ...l, key: e.target.value as SortByKey } : l
                          ))}
                          className="flex-1 bg-bg-base border border-border text-text-primary text-xs px-1.5 py-0.5 rounded focus:outline-none focus:border-accent cursor-pointer"
                        >
                          {SORT_OPTIONS.map(opt => (
                            <option key={opt.key} value={opt.key}>{opt.label}</option>
                          ))}
                        </select>
                        <button
                          onClick={() => setSortLevels(prev => prev.map((l, li) =>
                            li === i ? { ...l, dir: l.dir === 'asc' ? 'desc' : 'asc' } : l
                          ))}
                          className="text-xs border border-border text-text-muted rounded px-1.5 py-0.5 hover:border-accent w-7 text-center"
                          title="Toggle direction"
                        >
                          {level.dir === 'asc' ? '▲' : '▼'}
                        </button>
                        <button
                          onClick={() => setSortLevels(prev => prev.filter((_, li) => li !== i))}
                          className="text-xs text-text-muted hover:text-destructive w-4 text-center"
                          title="Remove level"
                        >
                          ×
                        </button>
                      </div>
                    ))}
                    <div className="px-2 pt-1.5 mt-0.5 border-t border-border">
                      <button
                        onClick={() => {
                          const usedKeys = new Set(sortLevels.map(l => l.key))
                          const nextKey = SORT_OPTIONS.find(o => !usedKeys.has(o.key))?.key ?? 'tracknumber'
                          setSortLevels(prev => [...prev, { key: nextKey, dir: 'asc' }])
                        }}
                        disabled={sortLevels.length >= SORT_OPTIONS.length}
                        className="text-xs text-accent hover:opacity-80 disabled:opacity-40 disabled:cursor-not-allowed"
                      >
                        + Add level
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Column headers */}
          <div className="flex items-center gap-0 px-2 py-1 bg-bg-panel border-b border-border text-text-muted text-[11px] uppercase tracking-wider flex-shrink-0">
            <span className="w-5 shrink-0 flex items-center">
              <input
                type="checkbox"
                checked={tracks.length > 0 && selectedTrackIds.size === tracks.length}
                ref={el => { if (el) el.indeterminate = selectedTrackIds.size > 0 && selectedTrackIds.size < tracks.length }}
                onChange={toggleSelectAll}
                className="accent-[color:var(--accent)] cursor-pointer"
                title="Select all"
              />
            </span>
            {COLUMNS.map(col => visibleColumns.has(col.key) && (
              <span key={col.key} className={col.className + ' shrink-0'}>
                {col.headerLabel ?? col.label}
              </span>
            ))}
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
                    <label key={col.key} className="flex items-center gap-2 px-3 py-1 hover:bg-bg-row-hover cursor-pointer">
                      <input
                        type="checkbox"
                        checked={visibleColumns.has(col.key)}
                        onChange={() => toggleColumn(col.key)}
                        className="accent-[color:var(--accent)]"
                      />
                      <span className="text-text-primary text-xs normal-case tracking-normal">{col.label}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Track list + bulk edit panel */}
          <div className="flex-1 flex flex-col overflow-hidden">
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
                displayGroups.map(({ key, tracks: groupTracks }) => (
                  <div key={key || '__all__'}>
                    {groupBy !== 'none' && (
                      <div className="px-3 py-1 bg-bg-panel border-b border-border text-[11px] font-mono flex items-center gap-2 sticky top-0 z-10">
                        <span className="text-text-primary font-medium">{key}</span>
                        <span className="text-text-muted/60">{groupTracks.length}</span>
                      </div>
                    )}
                    {groupTracks.map((track: Track) => (
                      <React.Fragment key={track.id}>
                        <TrackRow
                          track={track}
                          visibleColumns={visibleColumns}
                          suggestion={suggestionsByTrack[track.id]}
                          isSelected={selectedTrackIds.has(track.id)}
                          isShowingAlt={altPanelTrackId === track.id}
                          onRowClick={e => handleRowClick(track.id, e)}
                          onCheckboxChange={() => handleCheckboxChange(track.id)}
                          onContextMenu={e => handleContextMenu(e, track)}
                          onThreeDotsClick={(x, y) => handleThreeDotsClick(track, x, y)}
                          onCloseAlt={() => setAltPanelTrackId(null)}
                        />
                        {track.derived_tracks?.map(dt => (
                          <DerivedTrackRow
                            key={dt.id}
                            derived={dt}
                            visibleColumns={visibleColumns}
                          />
                        ))}
                      </React.Fragment>
                    ))}
                  </div>
                ))
              )}
            </div>

            {selectedTrackIds.size > 0 && (
              <BulkEditPanel
                key={[...selectedTrackIds].sort((a, b) => a - b).join(',')}
                selectedTracks={selectedTracks}
                onClose={() => setSelectedTrackIds(new Set())}
              />
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

      {contextMenu && (
        <ContextMenu x={contextMenu.x} y={contextMenu.y} items={contextMenu.items} />
      )}
    </div>
  )
}

// ── TrackRow ───────────────────────────────────────────────────────────────────
function TrackRow({
  track,
  visibleColumns,
  suggestion,
  isSelected,
  isShowingAlt,
  onRowClick,
  onCheckboxChange,
  onContextMenu,
  onThreeDotsClick,
  onCloseAlt,
}: {
  track: Track
  visibleColumns: Set<string>
  suggestion?: TagSuggestion
  isSelected: boolean
  isShowingAlt: boolean
  onRowClick: (e: React.MouseEvent) => void
  onCheckboxChange: () => void
  onContextMenu: (e: React.MouseEvent) => void
  onThreeDotsClick: (x: number, y: number) => void
  onCloseAlt: () => void
}) {
  const pct = suggestion ? Math.round(suggestion.confidence * 100) : null

  return (
    <>
      <div
        className={`flex items-center gap-0 px-2 py-0.5 border-b border-border-subtle text-xs hover:bg-bg-row-hover cursor-pointer select-none ${isSelected ? 'bg-accent/10' : ''}`}
        onClick={onRowClick}
        onContextMenu={onContextMenu}
      >
        <span className="w-5 shrink-0 flex items-center" onClick={e => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={isSelected}
            onChange={onCheckboxChange}
            className="accent-[color:var(--accent)] cursor-pointer"
          />
        </span>
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
              onClick={e => {
                e.stopPropagation()
                const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
                onThreeDotsClick(rect.left, rect.bottom + 4)
              }}
              className="text-xs border border-border text-text-muted rounded px-1.5 py-0.5 hover:border-accent hover:text-text-secondary transition-colors"
              title="Track actions"
            >
              ⋯
            </button>
          </span>
        )}
      </div>

      {isShowingAlt && suggestion?.alternatives && suggestion.alternatives.length > 0 && (
        <div className="border-b border-border bg-bg-surface px-3 py-2">
          <AlternativesPanel suggestion={suggestion} onClose={onCloseAlt} />
        </div>
      )}
    </>
  )
}

// ── DerivedTrackRow ────────────────────────────────────────────────────────────
function DerivedTrackRow({
  derived,
  visibleColumns,
}: {
  derived: Track
  visibleColumns: Set<string>
}) {
  // The first path segment is the derived-dir-name set by the library profile
  // e.g. "aac-192k/Artist/Album/track.m4a" → "aac-192k"
  const profileLabel = derived.relative_path.split('/')[0] ?? '—'

  return (
    <div className="flex items-center gap-0 px-2 py-0.5 border-b border-border-subtle text-xs text-text-muted/60 select-none bg-bg-base/40">
      {/* indent + connector in place of checkbox */}
      <span className="w-5 shrink-0 flex items-center justify-center text-text-muted/40 text-[10px]">↳</span>
      {visibleColumns.has('num') && (
        <span className="w-6 shrink-0" />
      )}
      {visibleColumns.has('title') && (
        <span className="flex-[3] shrink-0 truncate pr-2 font-mono text-[10px] text-text-muted/70">
          {profileLabel}
        </span>
      )}
      {visibleColumns.has('artist') && (
        <span className="flex-[2] shrink-0" />
      )}
      {visibleColumns.has('album') && (
        <span className="flex-[2] shrink-0" />
      )}
      {visibleColumns.has('year') && (
        <span className="w-10 shrink-0" />
      )}
      {visibleColumns.has('genre') && (
        <span className="flex-1 shrink-0" />
      )}
      {visibleColumns.has('format') && (
        <span className="w-12 shrink-0 font-mono uppercase text-[10px]">
          {getFileExtension(derived.relative_path)}
        </span>
      )}
      {visibleColumns.has('bitrate') && (
        <span className="w-14 shrink-0 font-mono">{formatBitrate(derived.bitrate)}</span>
      )}
      {visibleColumns.has('duration') && (
        <span className="w-10 shrink-0 font-mono">{formatDuration(derived.duration_secs)}</span>
      )}
      {visibleColumns.has('actions') && (
        <span className="w-16 shrink-0" />
      )}
    </div>
  )
}

// ── BulkEditPanel ──────────────────────────────────────────────────────────────
function BulkEditPanel({
  selectedTracks,
  onClose,
}: {
  selectedTracks: Track[]
  onClose: () => void
}) {
  const qc = useQueryClient()
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [savedCount, setSavedCount] = useState<number | null>(null)

  // Computed once on mount — key prop remounts on selection change
  const [{ initialValues, differsFields }] = useState(() => {
    const initials: Record<string, string> = {}
    const differs = new Set<string>()
    for (const { key } of BULK_EDIT_FIELDS) {
      const vals = selectedTracks.map(t => getTrackTagValue(t, key))
      const first = vals[0] ?? ''
      if (vals.length > 0 && vals.every(v => v === first)) {
        initials[key] = first
      } else {
        initials[key] = ''
        differs.add(key)
      }
    }
    return { initialValues: initials, differsFields: differs }
  })

  const [currentValues, setCurrentValues] = useState<Record<string, string>>(
    () => Object.fromEntries(BULK_EDIT_FIELDS.map(({ key }) => [key, initialValues[key]]))
  )

  function isDirty(key: string): boolean {
    if (differsFields.has(key)) return currentValues[key].trim() !== ''
    return currentValues[key] !== initialValues[key]
  }

  const hasDirty = BULK_EDIT_FIELDS.some(f => isDirty(f.key))

  async function handleApply() {
    const tags: Record<string, string> = {}
    for (const { key } of BULK_EDIT_FIELDS) {
      if (isDirty(key) && currentValues[key].trim() !== '') {
        tags[key] = currentValues[key].trim()
      }
    }
    if (Object.keys(tags).length === 0) return

    setSaving(true)
    setError(null)
    setSavedCount(null)
    let count = 0
    const errors: string[] = []
    for (const track of selectedTracks) {
      try {
        await tagSuggestionsApi.create({
          track_id: track.id,
          source: 'mb_search',
          suggested_tags: tags,
          confidence: 1.0,
        })
        count++
      } catch (e) {
        errors.push(e instanceof Error ? e.message : 'unknown error')
      }
    }
    setSaving(false)
    setSavedCount(count)
    if (errors.length > 0) setError(`${errors.length} failed: ${errors[0]}`)
    qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
    qc.invalidateQueries({ queryKey: ['inbox-count'] })
  }

  return (
    <div className="border-t border-border bg-bg-surface flex-shrink-0 flex flex-col overflow-hidden" style={{ maxHeight: '45vh' }}>
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border flex-shrink-0">
        <span className="text-xs text-text-muted">
          {selectedTracks.length} track{selectedTracks.length !== 1 ? 's' : ''} selected
        </span>
        <span className="text-[10px] text-text-muted truncate max-w-xs">
          {selectedTracks.map(t => t.title ?? t.relative_path.split('/').pop()).join(', ')}
        </span>
        <div className="ml-auto flex items-center gap-2">
          {savedCount != null && <span className="text-xs text-green-400">Applied to {savedCount}</span>}
          {error && <span className="text-xs text-destructive">{error}</span>}
          <button
            type="button"
            onClick={handleApply}
            disabled={saving || !hasDirty}
            className="text-xs bg-accent text-bg-base rounded px-3 py-0.5 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {saving ? 'Applying…' : 'Apply to Selected'}
          </button>
          <button
            type="button"
            onClick={onClose}
            className="text-xs text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5"
          >
            Clear
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-2">
        <div className="grid grid-cols-6 gap-x-3 gap-y-1.5">
          {BULK_EDIT_FIELDS.map(({ key, label, cols }) => (
            <label
              key={key}
              className={`flex flex-col gap-0.5 ${COL_SPAN[cols ?? 2] ?? 'col-span-2'}`}
            >
              <span className="text-text-muted text-[10px] uppercase tracking-wider">{label}</span>
              <input
                type="text"
                value={currentValues[key]}
                placeholder={differsFields.has(key) ? '(multiple values)' : ''}
                onChange={e => {
                  setSavedCount(null)
                  setCurrentValues(prev => ({ ...prev, [key]: e.target.value }))
                }}
                className={`bg-bg-base border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent font-mono placeholder:text-text-muted/40 ${
                  isDirty(key) ? 'border-accent/60' : 'border-border'
                }`}
              />
            </label>
          ))}
        </div>
      </div>
    </div>
  )
}

// ── ContextMenu ────────────────────────────────────────────────────────────────
function ContextMenu({ x, y, items }: { x: number; y: number; items: MenuItem[] }) {
  return (
    <div
      style={{ position: 'fixed', left: x, top: y }}
      className="z-[100] bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[140px]"
      onClick={e => e.stopPropagation()}
      onContextMenu={e => e.preventDefault()}
    >
      {items.map((item, i) =>
        item == null ? (
          <div key={i} className="border-t border-border my-1" />
        ) : (
          <button
            key={item.label}
            onClick={item.action}
            className="block w-full text-left px-3 py-1 text-xs text-text-primary hover:bg-bg-row-hover"
          >
            {item.label}
          </button>
        )
      )}
    </div>
  )
}
