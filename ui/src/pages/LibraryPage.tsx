import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { LibraryTree, type BrowseMode } from '../components/LibraryTree'
import { AlternativesPanel } from '../components/AlternativesPanel'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { useAuth } from '../contexts/AuthContext'
import { getLibrary, listLibraries, listLibraryTracks, triggerMaintenance } from '../api/libraries'
import { enqueueLookup, scheduleDelete } from '../api/tracks'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import { artApi } from '../api/art'
import { getJob } from '../api/jobs'
import client from '../api/client'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'
import { useUserPrefs, DEFAULT_COL_WIDTHS } from '../hooks/useUserPrefs'
import type { GroupByKey, SortByKey } from '../hooks/useUserPrefs'
import { Checkbox } from '../components/Checkbox'

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

// Quick label lookup for suggestion review pane — covers all keys emitted by to_tag_map
const FIELD_LABELS: Record<string, string> = Object.fromEntries(
  BULK_EDIT_FIELDS.map(({ key, label }) => [key, label])
)

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
interface ColumnDef { key: string; label: string; headerLabel?: string }

const COLUMNS: ColumnDef[] = [
  { key: 'num',      label: 'Track #',  headerLabel: '#'   },
  { key: 'title',    label: 'Title'                        },
  { key: 'artist',   label: 'Artist'                       },
  { key: 'album',    label: 'Album'                        },
  { key: 'year',     label: 'Year'                         },
  { key: 'genre',    label: 'Genre'                        },
  { key: 'format',   label: 'Format'                       },
  { key: 'bitrate',  label: 'Quality'                      },
  { key: 'duration', label: 'Duration', headerLabel: 'Time'},
  { key: 'actions',  label: 'Actions'                      },
]

// Checkbox column width (fixed, never resizable)
const CB_COL_WIDTH = 24


function formatDuration(secs?: number): string {
  if (secs == null) return '—'
  const m = Math.floor(secs / 60)
  const s = Math.floor(secs % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

function formatQuality(bitrate?: number, bitDepth?: number, sampleRate?: number): string {
  if (bitDepth != null) {
    const khz = sampleRate != null ? (sampleRate / 1000).toFixed(sampleRate % 1000 === 0 ? 0 : 1) : null
    return khz != null ? `${bitDepth}-bit / ${khz}kHz` : `${bitDepth}-bit`
  }
  if (bitrate != null) return `${bitrate}k`
  return '—'
}

function getFileExtension(path: string): string {
  const dot = path.lastIndexOf('.')
  if (dot === -1) return '—'
  return path.slice(dot + 1).toLowerCase()
}

// ── Sort / group types ─────────────────────────────────────────────────────────
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
  const [maintenanceQueued, setMaintenanceQueued] = useState(false)
  const {
    groupBy, setGroupBy,
    sortLevels, setSortLevels,
    colWidths, setColWidths,
    visibleCols: visibleColumns, toggleColumn,
    editPanelHeight, setEditPanelHeight,
  } = useUserPrefs()
  // liveWidths tracks widths during drag without persisting on every pixel move
  const [liveWidths, setLiveWidths] = useState<Record<string, number>>(colWidths)
  useEffect(() => { setLiveWidths(colWidths) }, [colWidths])
  const resizingRef = useRef<{ key: string; startX: number; startWidth: number } | null>(null)

  // Edit panel height resize
  const [liveEditPanelHeight, setLiveEditPanelHeight] = useState(editPanelHeight)
  useEffect(() => { setLiveEditPanelHeight(editPanelHeight) }, [editPanelHeight])
  const panelResizeRef = useRef<{ startY: number; startH: number } | null>(null)

  const handlePanelResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    panelResizeRef.current = { startY: e.clientY, startH: liveEditPanelHeight }
    const onMove = (ev: MouseEvent) => {
      if (!panelResizeRef.current) return
      const delta = panelResizeRef.current.startY - ev.clientY
      const next = Math.max(160, Math.min(window.innerHeight * 0.8, panelResizeRef.current.startH + delta))
      setLiveEditPanelHeight(next)
    }
    const onUp = (ev: MouseEvent) => {
      if (!panelResizeRef.current) return
      const delta = panelResizeRef.current.startY - ev.clientY
      const next = Math.max(160, Math.min(window.innerHeight * 0.8, panelResizeRef.current.startH + delta))
      setEditPanelHeight(next)
      panelResizeRef.current = null
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }, [liveEditPanelHeight, setEditPanelHeight])
  const [showColumnPicker, setShowColumnPicker] = useState(false)
  const pickerRef = useRef<HTMLDivElement>(null)
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)
  const [selectedTrackIds, setSelectedTrackIds] = useState<Set<number>>(new Set())
  const lastSelectedIdRef = useRef<number | null>(null)
  const [showGroupMenu, setShowGroupMenu] = useState(false)
  const [showSortMenu, setShowSortMenu] = useState(false)
  const groupMenuRef = useRef<HTMLDivElement>(null)
  const sortMenuRef = useRef<HTMLDivElement>(null)
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null)
  const [altPanelTrackId, setAltPanelTrackId] = useState<number | null>(null)
  const [browseMode, setBrowseMode] = useState<BrowseMode | null>(null)
  const [browseFilter, setBrowseFilter] = useState<string | null>(null)
  const [showActionsMenu, setShowActionsMenu] = useState(false)
  const actionsMenuRef = useRef<HTMLDivElement>(null)
  // deleteConfirm: null = closed; object = open with context
  const [deleteConfirm, setDeleteConfirm] = useState<{
    ids: number[]
    label: string          // e.g. "3 tracks" or "Album — Dark Side of the Moon (10 tracks)"
  } | null>(null)
  const [deleteSubmitting, setDeleteSubmitting] = useState(false)
  const [artUpdateModal, setArtUpdateModal] = useState<{ trackIds: number[]; label: string } | null>(null)

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
    if (!showGroupMenu && !showSortMenu && !showActionsMenu) return
    function handleMouseDown(e: MouseEvent) {
      if (showGroupMenu && groupMenuRef.current && !groupMenuRef.current.contains(e.target as Node)) {
        setShowGroupMenu(false)
      }
      if (showSortMenu && sortMenuRef.current && !sortMenuRef.current.contains(e.target as Node)) {
        setShowSortMenu(false)
      }
      if (showActionsMenu && actionsMenuRef.current && !actionsMenuRef.current.contains(e.target as Node)) {
        setShowActionsMenu(false)
      }
    }
    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [showGroupMenu, showSortMenu, showActionsMenu])

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

  // ── Column resize handlers ────────────────────────────────────────────────────
  const handleResizeMouseMove = useRef((e: MouseEvent) => {
    const r = resizingRef.current
    if (!r) return
    const newWidth = Math.max(40, r.startWidth + (e.clientX - r.startX))
    setLiveWidths(prev => ({ ...prev, [r.key]: newWidth }))
  }).current

  const handleResizeMouseUp = useRef(() => {
    document.removeEventListener('mousemove', handleResizeMouseMove)
    document.removeEventListener('mouseup', handleResizeMouseUp)
    document.body.style.userSelect = ''
    setLiveWidths(prev => {
      setColWidths(prev)
      return prev
    })
    resizingRef.current = null
  }).current

  function handleResizeMouseDown(key: string, e: React.MouseEvent) {
    e.preventDefault()
    resizingRef.current = {
      key,
      startX: e.clientX,
      startWidth: liveWidths[key] ?? DEFAULT_COL_WIDTHS[key] ?? 80,
    }
    document.body.style.userSelect = 'none'
    document.addEventListener('mousemove', handleResizeMouseMove)
    document.addEventListener('mouseup', handleResizeMouseUp)
  }

  // ── Queries ───────────────────────────────────────────────────────────────────
  const { data: libraries = [] } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  // Auto-select default library on first load
  useEffect(() => {
    if (selectedLibraryId != null || selectedVirtualLibraryId != null) return
    const def = libraries.find(l => l.is_default)
    if (def) setSelectedLibraryId(def.id)
  }, [libraries, selectedLibraryId, selectedVirtualLibraryId])

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

  // ── Browse helpers ─────────────────────────────────────────────────────────────
  function getBrowseValue(track: Track, mode: BrowseMode): string {
    switch (mode) {
      case 'artist':      return track.artist ?? ''
      case 'albumartist': return track.albumartist ?? ''
      case 'album':       return track.album ?? ''
      case 'genre':       return track.genre ?? ''
    }
  }

  const BROWSE_LABEL: Record<BrowseMode, string> = {
    artist:      'Artist',
    albumartist: 'Album Artist',
    album:       'Album',
    genre:       'Genre',
  }

  const browseValues = useMemo(() => {
    if (!browseMode) return []
    const counts = new Map<string, number>()
    for (const t of tracks) {
      const v = getBrowseValue(t, browseMode) || '—'
      counts.set(v, (counts.get(v) ?? 0) + 1)
    }
    return [...counts.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([val, count]) => ({ val, count }))
  }, [tracks, browseMode])

  // Tracks scoped by active browse filter (State C), or all tracks otherwise
  const displayTracks = useMemo(() => {
    if (!browseMode || !browseFilter) return tracks
    return tracks.filter(t => (getBrowseValue(t, browseMode) || '—') === browseFilter)
  }, [tracks, browseMode, browseFilter])

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
      return [{ key: '', tracks: sortTracks(displayTracks) }]
    }
    const groupMap = new Map<string, Track[]>()
    for (const t of displayTracks) {
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
    if (selectedTrackIds.size === displayTracks.length && displayTracks.length > 0) {
      setSelectedTrackIds(new Set())
    } else {
      setSelectedTrackIds(new Set(displayTracks.map((t: Track) => t.id)))
    }
  }

  // ── Context menu builders ─────────────────────────────────────────────────────
  function handleContextMenu(e: React.MouseEvent, track: Track) {
    e.preventDefault()
    e.stopPropagation()
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: 'Identify via AcoustID',       action: () => { lookupMutation.mutate(track.id); setContextMenu(null) } },
        { label: 'Search MusicBrainz / FreeDB…', action: () => { setSearchTrack(track); setContextMenu(null) } },
        null,
        { label: 'Select All',   action: () => { toggleSelectAll(); setContextMenu(null) } },
        { label: 'Deselect All', action: () => { setSelectedTrackIds(new Set()); setContextMenu(null) } },
        null,
        { label: 'Copy Path', action: () => { navigator.clipboard.writeText(track.relative_path); setContextMenu(null) } },
        null,
        { label: 'Delete track…', action: () => { setDeleteConfirm({ ids: [track.id], label: '1 track' }); setContextMenu(null) } },
      ],
    })
  }

  function handleThreeDotsClick(track: Track, x: number, y: number) {
    const suggestion = suggestionsByTrack[track.id]
    const items: MenuItem[] = [
      { label: 'Identify via AcoustID',        action: () => { lookupMutation.mutate(track.id); setContextMenu(null) } },
      { label: 'Search MusicBrainz / FreeDB…', action: () => { setSearchTrack(track); setContextMenu(null) } },
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
    items.push(null)
    items.push({ label: 'Update art…', action: () => { setArtUpdateModal({ trackIds: [track.id], label: track.title ?? track.relative_path.split('/').pop() ?? '1 track' }); setContextMenu(null) } })
    items.push({ label: 'Delete track…', action: () => { setDeleteConfirm({ ids: [track.id], label: '1 track' }); setContextMenu(null) } })
    setContextMenu({ x, y, items })
  }

  // ── Mutations ─────────────────────────────────────────────────────────────────
  const acceptMutation = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.accept(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['ingest-count'] })
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

  async function handleBulkLookup(ids: number[]) {
    setShowActionsMenu(false)
    for (const id of ids) {
      await enqueueLookup(id).catch(() => {})
    }
  }

  async function handleConfirmDelete() {
    if (!deleteConfirm) return
    setDeleteSubmitting(true)
    try {
      await scheduleDelete(deleteConfirm.ids)
      setDeleteConfirm(null)
      setSelectedTrackIds(new Set())
      qc.invalidateQueries({ queryKey: ['library-tracks', selectedLibraryId] })
    } catch { /* ignore */ }
    setDeleteSubmitting(false)
  }

  async function handleScan() {
    if (selectedLibraryId == null) return
    try {
      const res = await client.post<{ id: number }>('/jobs/scan', { library_id: selectedLibraryId })
      const jobId = res.data.id
      setScanQueued(true)
      const libId = selectedLibraryId
      const timer = setInterval(async () => {
        try {
          const job = await getJob(jobId)
          if (job.status === 'completed' || job.status === 'failed') {
            clearInterval(timer)
            setScanQueued(false)
            qc.invalidateQueries({ queryKey: ['library-tracks', libId] })
          }
        } catch { clearInterval(timer); setScanQueued(false) }
      }, 2000)
    } catch { /* ignore */ }
  }

  async function handleMaintenance() {
    if (selectedLibraryId == null) return
    try {
      const { job_id } = await triggerMaintenance(selectedLibraryId)
      setMaintenanceQueued(true)
      const libId = selectedLibraryId
      const timer = setInterval(async () => {
        try {
          const job = await getJob(job_id)
          if (job.status === 'completed' || job.status === 'failed') {
            clearInterval(timer)
            setMaintenanceQueued(false)
            qc.invalidateQueries({ queryKey: ['library-tracks', libId] })
          }
        } catch { clearInterval(timer); setMaintenanceQueued(false) }
      }, 2000)
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
            onSelectLibrary={id => {
              setSelectedLibraryId(id)
              setSelectedVirtualLibraryId(null)
              setAltPanelTrackId(null)
              setBrowseMode(null)
              setBrowseFilter(null)
              setSelectedTrackIds(new Set())
            }}
            selectedVirtualLibraryId={selectedVirtualLibraryId}
            onSelectVirtualLibrary={id => {
              setSelectedVirtualLibraryId(id)
              setSelectedLibraryId(null)
              setAltPanelTrackId(null)
              setBrowseMode(null)
              setBrowseFilter(null)
              setSelectedTrackIds(new Set())
            }}
            selectedBrowseMode={browseMode}
            onSelectBrowseMode={(libraryId, mode) => {
              setSelectedLibraryId(libraryId)
              setSelectedVirtualLibraryId(null)
              setAltPanelTrackId(null)
              setBrowseMode(mode)
              setBrowseFilter(null)
              setSelectedTrackIds(new Set())
            }}
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
                  {scanQueued && <span className="text-xs text-accent mr-1">Scanning…</span>}
                  {maintenanceQueued && <span className="text-xs text-accent mr-1">Maintaining…</span>}
                  <button
                    onClick={handleScan}
                    className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:text-text-primary hover:border-accent"
                    title="Scan this library for new/changed files"
                  >
                    Scan
                  </button>
                  <button
                    onClick={handleMaintenance}
                    className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:text-text-primary hover:border-accent"
                    title="Re-read audio properties and mark missing files as removed"
                  >
                    Maintain
                  </button>
                </>
              )}
              {/* Actions dropdown — visible when tracks selected */}
              {selectedTrackIds.size > 0 && (
                <div ref={actionsMenuRef} className="relative">
                  <button
                    onClick={() => setShowActionsMenu(v => !v)}
                    className={`text-xs bg-bg-panel border rounded px-2 py-0.5 ${showActionsMenu ? 'border-accent text-accent' : 'border-border text-text-muted hover:text-text-primary'}`}
                  >
                    Actions ({selectedTrackIds.size}) ▾
                  </button>
                  {showActionsMenu && (
                    <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[180px]">
                      <button
                        onClick={() => handleBulkLookup([...selectedTrackIds])}
                        className="block w-full text-left px-3 py-1 text-xs text-text-primary hover:bg-bg-row-hover"
                      >
                        AcoustID Lookup ({selectedTrackIds.size})
                      </button>
                      <div className="border-t border-border my-1" />
                      <button
                        onClick={() => {
                          const ids = [...selectedTrackIds]
                          setDeleteConfirm({ ids, label: `${ids.length} track${ids.length !== 1 ? 's' : ''}` })
                          setShowActionsMenu(false)
                        }}
                        className="block w-full text-left px-3 py-1 text-xs text-destructive hover:bg-bg-row-hover"
                      >
                        Delete {selectedTrackIds.size} track{selectedTrackIds.size !== 1 ? 's' : ''}…
                      </button>
                    </div>
                  )}
                </div>
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

          {/* Breadcrumb bar — shown in State C (browse mode + filter active) */}
          {browseMode && browseFilter && (
            <div className="flex items-center gap-2 px-3 py-2 bg-bg-surface border-b border-border flex-shrink-0">
              <span className="text-xs text-text-muted">{BROWSE_LABEL[browseMode]}</span>
              <span className="text-text-muted/50 text-xs">›</span>
              <span className="text-xs text-text-primary font-medium truncate">{browseFilter}</span>
              <span className="text-xs text-text-muted/60 ml-1">({displayTracks.length})</span>
              <button
                onClick={() => setBrowseFilter(null)}
                className="ml-auto text-xs text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5 shrink-0"
                title="Back to browse list"
              >
                ← Back
              </button>
            </div>
          )}

          {/* Column headers — hidden in State B (browse list) */}
          {!(browseMode && !browseFilter) && (
          <div className="flex items-stretch gap-0 bg-bg-panel border-b border-border text-text-muted text-[11px] uppercase tracking-wider flex-shrink-0 select-none">
            {/* Checkbox column — fixed, not resizable */}
            <div style={{ width: CB_COL_WIDTH, flexShrink: 0 }} className="flex items-center justify-center py-1">
              <Checkbox
                checked={displayTracks.length > 0 && selectedTrackIds.size === displayTracks.length}
                indeterminate={selectedTrackIds.size > 0 && selectedTrackIds.size < displayTracks.length}
                onChange={toggleSelectAll}
                title="Select all"
              />
            </div>
            {COLUMNS.map(col => visibleColumns.has(col.key) && (
              <div
                key={col.key}
                className="relative flex items-center justify-center py-1 border-l border-border hover:bg-bg-row-hover group"
                style={{ width: liveWidths[col.key] ?? DEFAULT_COL_WIDTHS[col.key], flexShrink: 0 }}
              >
                <span className="truncate px-2">{col.headerLabel ?? col.label}</span>
                {col.key !== 'actions' && (
                  <div
                    className="absolute right-0 top-0 bottom-0 w-2 cursor-col-resize flex items-center justify-center text-text-muted/40 group-hover:text-accent z-10"
                    onMouseDown={e => handleResizeMouseDown(col.key, e)}
                  >
                    ⋮
                  </div>
                )}
              </div>
            ))}
            <div ref={pickerRef} className="relative flex items-center justify-center border-l border-border ml-auto" style={{ width: 24, flexShrink: 0 }}>
              <span
                className="text-accent cursor-pointer hover:opacity-70 transition-opacity"
                onClick={() => setShowColumnPicker(v => !v)}
                title="Customize columns"
              >
                ⊕
              </span>
              {showColumnPicker && (
                <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[140px]">
                  {COLUMNS.map(col => (
                    <label key={col.key} className="flex items-center gap-2 px-3 py-1 hover:bg-bg-row-hover cursor-pointer">
                      <Checkbox
                        checked={visibleColumns.has(col.key)}
                        onChange={() => toggleColumn(col.key)}
                      />
                      <span className="text-text-primary text-xs normal-case tracking-normal">{col.label}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>
          )} {/* end column headers conditional */}

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
              ) : browseMode && !browseFilter ? (
                /* ── State B: Browse value list ─────────────────────────────── */
                <div>
                  <div className="flex items-center px-3 py-1 bg-bg-panel border-b border-border text-[11px] uppercase tracking-wider text-text-muted flex-shrink-0">
                    <span className="flex-1">{BROWSE_LABEL[browseMode]}</span>
                    <span className="w-16 text-right">Tracks</span>
                  </div>
                  {browseValues.length === 0 ? (
                    <div className="flex items-center justify-center h-32 text-text-muted text-xs">
                      No tracks in this library. Run a scan to discover files.
                    </div>
                  ) : browseValues.map(({ val, count }) => (
                    <div
                      key={val}
                      className="flex items-center px-3 py-1.5 border-b border-border-subtle hover:bg-bg-row-hover cursor-pointer select-none"
                      onClick={() => setBrowseFilter(val)}
                    >
                      <span className="flex-1 text-xs text-text-primary truncate pr-4">{val}</span>
                      <span className="text-xs text-text-muted font-mono w-16 text-right shrink-0">{count}</span>
                    </div>
                  ))}
                </div>
              ) : tracks.length === 0 ? (
                <div className="flex items-center justify-center h-32 text-text-muted text-xs">
                  No tracks in this library. Run a scan to discover files.
                </div>
              ) : (
                /* ── State A / C: Track list ────────────────────────────────── */
                displayGroups.map(({ key, tracks: groupTracks }) => (
                  <div key={key || '__all__'}>
                    {groupBy !== 'none' && (() => {
                      const groupIds = groupTracks.map((t: Track) => t.id)
                      const allSelected = groupIds.length > 0 && groupIds.every(id => selectedTrackIds.has(id))
                      const someSelected = groupIds.some(id => selectedTrackIds.has(id))
                      const showCheckbox = groupBy === 'album' || groupBy === 'artist' || groupBy === 'albumartist'
                      const deleteLabel = { album: 'Delete album…', artist: 'Delete artist…', albumartist: 'Delete album artist…' }[groupBy as string] ?? 'Delete group…'
                      const artTrack = groupTracks.find((t: Track) => t.has_embedded_art)
                      const artSuggestion = groupTracks.map((t: Track) => suggestionsByTrack[t.id]).find(s => s?.cover_art_url)
                      const artSrc = artTrack
                        ? `/api/v1/tracks/${artTrack.id}/art`
                        : (artSuggestion?.cover_art_url ?? null)
                      return (
                        <div className="bg-bg-panel border-b border-border text-xs font-mono flex items-center sticky top-0 z-10">
                          <span style={{ width: CB_COL_WIDTH, flexShrink: 0 }} className="flex items-center justify-center py-0.5">
                            {showCheckbox && (
                              <Checkbox
                                checked={allSelected}
                                indeterminate={someSelected && !allSelected}
                                onChange={() => {
                                  setSelectedTrackIds(prev => {
                                    const next = new Set(prev)
                                    if (allSelected) groupIds.forEach(id => next.delete(id))
                                    else groupIds.forEach(id => next.add(id))
                                    return next
                                  })
                                }}
                                title="Select all in group"
                              />
                            )}
                          </span>
                          {artSrc ? (
                            <img src={artSrc} alt="" className="w-8 h-8 object-cover flex-shrink-0 mr-1.5" />
                          ) : (
                            <span className="w-8 h-8 flex-shrink-0 mr-1.5" />
                          )}
                          <span className="text-text-primary font-medium py-0.5">{key}</span>
                          <span className="text-text-muted/60 ml-2 py-0.5">{groupTracks.length}</span>
                          <button
                            className="ml-auto mr-1 border border-border text-text-muted rounded px-1.5 py-0 hover:border-accent hover:text-text-secondary transition-colors leading-none"
                            onClick={e => {
                              e.stopPropagation()
                              const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
                              setContextMenu({
                                x: rect.left,
                                y: rect.bottom + 4,
                                items: [
                                  { label: 'Update art…', action: () => {
                                    setArtUpdateModal({ trackIds: groupIds, label: key })
                                    setContextMenu(null)
                                  }},
                                  { label: deleteLabel, action: () => {
                                    setDeleteConfirm({ ids: groupIds, label: `${key} (${groupIds.length} track${groupIds.length !== 1 ? 's' : ''})` })
                                    setContextMenu(null)
                                  }},
                                ],
                              })
                            }}
                            title="Group actions"
                          >⋯</button>
                        </div>
                      )
                    })()}
                    {groupTracks.map((track: Track) => (
                      <React.Fragment key={track.id}>
                        <TrackRow
                          track={track}
                          visibleColumns={visibleColumns}
                          colWidths={liveWidths}
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
                            colWidths={liveWidths}
                          />
                        ))}
                      </React.Fragment>
                    ))}
                  </div>
                ))
              )}
            </div>

            {selectedTrackIds.size > 0 && (
              <div className="flex-shrink-0 flex flex-col" style={{ height: liveEditPanelHeight }}>
                {/* Drag handle */}
                <div
                  className="h-1.5 flex-shrink-0 cursor-row-resize bg-transparent hover:bg-accent/20 active:bg-accent/30 transition-colors"
                  onMouseDown={handlePanelResizeStart}
                />
                <BulkEditPanel
                  key={[...selectedTrackIds].sort((a, b) => a - b).join(',')}
                  selectedTracks={selectedTracks}
                  suggestionsByTrack={suggestionsByTrack}
                  onClose={() => setSelectedTrackIds(new Set())}
                  onSuggestionActioned={() => {
                    qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
                    qc.invalidateQueries({ queryKey: ['library-tracks', selectedLibraryId] })
                  }}
                />
              </div>
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

      {deleteConfirm && (
        <DeleteConfirmModal
          label={deleteConfirm.label}
          submitting={deleteSubmitting}
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={handleConfirmDelete}
        />
      )}
      {artUpdateModal && (
        <ArtUpdateModal
          trackIds={artUpdateModal.trackIds}
          label={artUpdateModal.label}
          onClose={() => setArtUpdateModal(null)}
        />
      )}
    </div>
  )
}

// ── TrackRow ───────────────────────────────────────────────────────────────────
function TrackRow({
  track,
  visibleColumns,
  colWidths,
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
  colWidths: Record<string, number>
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
  const w = (key: string) => ({ width: colWidths[key] ?? DEFAULT_COL_WIDTHS[key], flexShrink: 0, overflow: 'hidden' })

  return (
    <>
      <div
        className={`flex items-center gap-0 border-b border-border-subtle text-xs hover:bg-bg-row-hover cursor-pointer select-none ${isSelected ? 'bg-accent/10' : ''}`}
        onClick={onRowClick}
        onContextMenu={onContextMenu}
      >
        <span style={{ width: CB_COL_WIDTH, flexShrink: 0 }} className="flex items-center justify-center py-0.5" onClick={e => e.stopPropagation()}>
          <Checkbox checked={isSelected} onChange={onCheckboxChange} />
        </span>
        {visibleColumns.has('num') && (
          <span style={w('num')} className="py-0.5 text-text-muted font-mono">{track.tracknumber ?? '—'}</span>
        )}
        {visibleColumns.has('title') && (
          <span style={w('title')} className="py-0.5 text-text-primary truncate px-1">{track.title ?? '—'}</span>
        )}
        {visibleColumns.has('artist') && (
          <span style={w('artist')} className="py-0.5 text-text-secondary truncate px-1">{track.artist ?? '—'}</span>
        )}
        {visibleColumns.has('album') && (
          <span style={w('album')} className="py-0.5 text-text-secondary truncate px-1">{track.album ?? '—'}</span>
        )}
        {visibleColumns.has('year') && (
          <span style={w('year')} className="py-0.5 text-text-muted">{track.date?.slice(0, 4) ?? '—'}</span>
        )}
        {visibleColumns.has('genre') && (
          <span style={w('genre')} className="py-0.5 text-text-muted truncate px-1">{track.genre ?? '—'}</span>
        )}
        {visibleColumns.has('format') && (
          <span style={w('format')} className="py-0.5 text-text-muted font-mono uppercase text-[10px]">
            {getFileExtension(track.relative_path)}
          </span>
        )}
        {visibleColumns.has('bitrate') && (
          <span style={w('bitrate')} className="py-0.5 text-text-muted font-mono text-[11px]">{formatQuality(track.bitrate, track.bit_depth, track.sample_rate)}</span>
        )}
        {visibleColumns.has('duration') && (
          <span style={w('duration')} className="py-0.5 text-text-muted font-mono">{formatDuration(track.duration_secs)}</span>
        )}
        {visibleColumns.has('actions') && (
          <span style={w('actions')} className="py-0.5 flex items-center gap-1 justify-end pr-1">
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
  colWidths,
}: {
  derived: Track
  visibleColumns: Set<string>
  colWidths: Record<string, number>
}) {
  // The first path segment is the derived-dir-name set by the library profile
  // e.g. "aac-192k/Artist/Album/track.m4a" → "aac-192k"
  const profileLabel = derived.relative_path.split('/')[0] ?? '—'
  const w = (key: string) => ({ width: colWidths[key] ?? DEFAULT_COL_WIDTHS[key], flexShrink: 0, overflow: 'hidden' })

  return (
    <div className="flex items-center gap-0 border-b border-border-subtle text-xs text-text-muted/60 select-none bg-bg-base/40">
      {/* indent + connector in place of checkbox */}
      <span style={{ width: CB_COL_WIDTH, flexShrink: 0 }} className="flex items-center justify-center py-0.5 text-text-muted/40 text-[10px]">↳</span>
      {visibleColumns.has('num') && <span style={w('num')} className="py-0.5" />}
      {visibleColumns.has('title') && (
        <span style={w('title')} className="py-0.5 truncate px-1 font-mono text-[10px] text-text-muted/70">
          {profileLabel}
        </span>
      )}
      {visibleColumns.has('artist')   && <span style={w('artist')}   className="py-0.5" />}
      {visibleColumns.has('album')    && <span style={w('album')}    className="py-0.5" />}
      {visibleColumns.has('year')     && <span style={w('year')}     className="py-0.5" />}
      {visibleColumns.has('genre')    && <span style={w('genre')}    className="py-0.5" />}
      {visibleColumns.has('format') && (
        <span style={w('format')} className="py-0.5 font-mono uppercase text-[10px]">
          {getFileExtension(derived.relative_path)}
        </span>
      )}
      {visibleColumns.has('bitrate') && (
        <span style={w('bitrate')} className="py-0.5 font-mono text-[11px]">{formatQuality(derived.bitrate, derived.bit_depth, derived.sample_rate)}</span>
      )}
      {visibleColumns.has('duration') && (
        <span style={w('duration')} className="py-0.5 font-mono">{formatDuration(derived.duration_secs)}</span>
      )}
      {visibleColumns.has('actions') && <span style={w('actions')} className="py-0.5" />}
    </div>
  )
}

// ── BulkEditPanel ──────────────────────────────────────────────────────────────
function BulkEditPanel({
  selectedTracks,
  suggestionsByTrack,
  onClose,
  onSuggestionActioned,
}: {
  selectedTracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  onClose: () => void
  onSuggestionActioned: () => void
}) {
  const qc = useQueryClient()
  const [activeTab, setActiveTab] = useState<'edit' | 'suggestion'>('edit')
  const [reviewTrackId, setReviewTrackId] = useState<number | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [savedCount, setSavedCount] = useState<number | null>(null)

  const tracksWithSuggestions = selectedTracks.filter(t => suggestionsByTrack[t.id])

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
    setSaving(true); setError(null); setSavedCount(null)
    let count = 0
    const errors: string[] = []
    for (const track of selectedTracks) {
      try {
        await tagSuggestionsApi.create({ track_id: track.id, source: 'mb_search', suggested_tags: tags, confidence: 1.0 })
        count++
      } catch (e) {
        errors.push(e instanceof Error ? e.message : 'unknown error')
      }
    }
    setSaving(false); setSavedCount(count)
    if (errors.length > 0) setError(`${errors.length} failed: ${errors[0]}`)
    qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
    qc.invalidateQueries({ queryKey: ['ingest-count'] })
  }

  // Resolve the track/suggestion to review (single-select or drill-in)
  const reviewTrack = reviewTrackId != null
    ? (selectedTracks.find(t => t.id === reviewTrackId) ?? null)
    : selectedTracks.length === 1 && tracksWithSuggestions.length === 1
      ? tracksWithSuggestions[0]
      : null
  const reviewSuggestion = reviewTrack ? suggestionsByTrack[reviewTrack.id] : null

  return (
    <div className="border-t border-border bg-bg-surface flex flex-col overflow-hidden h-full">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border flex-shrink-0">
        {/* Tabs */}
        <div className="flex gap-0 border border-border rounded overflow-hidden shrink-0">
          <button
            type="button"
            onClick={() => setActiveTab('edit')}
            className={`text-xs px-2.5 py-0.5 transition-colors ${activeTab === 'edit' ? 'bg-accent text-bg-base' : 'text-text-muted hover:text-text-primary'}`}
          >
            Edit
          </button>
          {tracksWithSuggestions.length > 0 && (
            <button
              type="button"
              onClick={() => { setActiveTab('suggestion'); setReviewTrackId(null) }}
              className={`text-xs px-2.5 py-0.5 border-l border-border transition-colors ${activeTab === 'suggestion' ? 'bg-accent text-bg-base' : 'text-text-muted hover:text-text-primary'}`}
            >
              Suggestion{tracksWithSuggestions.length > 1 ? ` (${tracksWithSuggestions.length})` : ''}
            </button>
          )}
        </div>

        <span className="text-xs text-text-muted">
          {selectedTracks.length} track{selectedTracks.length !== 1 ? 's' : ''}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {activeTab === 'edit' && (
            <>
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
            </>
          )}
          <button type="button" onClick={onClose} className="text-xs text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5">
            Close
          </button>
        </div>
      </div>

      {/* Tab content */}
      {activeTab === 'edit' ? (
        <div className="flex-1 overflow-y-auto px-3 py-2">
          <div className="grid grid-cols-6 gap-x-3 gap-y-1.5">
            {BULK_EDIT_FIELDS.map(({ key, label, cols }) => (
              <label key={key} className={`flex flex-col gap-0.5 ${COL_SPAN[cols ?? 2] ?? 'col-span-2'}`}>
                <span className="text-text-muted text-[10px] uppercase tracking-wider">{label}</span>
                <input
                  type="text"
                  value={currentValues[key]}
                  placeholder={differsFields.has(key) ? '(multiple values)' : ''}
                  onChange={e => { setSavedCount(null); setCurrentValues(prev => ({ ...prev, [key]: e.target.value })) }}
                  className={`bg-bg-base border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent font-mono placeholder:text-text-muted/40 ${isDirty(key) ? 'border-accent/60' : 'border-border'}`}
                />
              </label>
            ))}
          </div>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          {reviewTrack && reviewSuggestion ? (
            /* Single-track diff view */
            <SuggestionReviewPane
              key={reviewTrack.id}
              track={reviewTrack}
              suggestion={reviewSuggestion}
              showBack={reviewTrackId != null}
              onBack={() => setReviewTrackId(null)}
              onDone={() => { onSuggestionActioned(); setReviewTrackId(null) }}
            />
          ) : (
            /* Multi-track list */
            <div className="flex flex-col">
              <div className="px-3 py-1.5 border-b border-border text-[11px] uppercase tracking-wider text-text-muted flex items-center">
                <span className="flex-1">Track</span>
                <span className="w-12 text-right">Match</span>
              </div>
              {tracksWithSuggestions.map(t => {
                const s = suggestionsByTrack[t.id]
                const pct = Math.round(s.confidence * 100)
                return (
                  <div key={t.id} className="flex items-center px-3 py-1.5 border-b border-border-subtle hover:bg-bg-row-hover">
                    <span className="flex-1 text-xs text-text-primary truncate pr-2">{t.title ?? t.relative_path.split('/').pop()}</span>
                    <span className={`text-xs font-mono mr-3 ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>{pct}%</span>
                    <button
                      onClick={() => setReviewTrackId(t.id)}
                      className="text-xs text-accent hover:opacity-80 shrink-0"
                    >
                      Review ▶
                    </button>
                  </div>
                )
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ── SuggestionReviewPane ───────────────────────────────────────────────────────
function SuggestionReviewPane({
  track,
  suggestion,
  showBack,
  onBack,
  onDone,
}: {
  track: Track
  suggestion: TagSuggestion
  showBack: boolean
  onBack: () => void
  onDone: () => void
}) {
  const qc = useQueryClient()

  const diffItems = useMemo(() => {
    const suggested = (suggestion.suggested_tags ?? {}) as Record<string, unknown>
    return Object.entries(suggested).map(([key, raw]) => {
      const suggestedVal = typeof raw === 'string' ? raw : String(raw ?? '')
      const currentVal = getTrackTagValue(track, key)
      return { key, currentVal, suggestedVal, changed: currentVal !== suggestedVal }
    })
  }, [suggestion, track])

  const [checkedFields, setCheckedFields] = useState<Set<string>>(
    () => new Set(diffItems.filter(d => d.changed).map(d => d.key))
  )
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [applyArt, setApplyArt] = useState(() => !!suggestion.cover_art_url)

  function toggleField(key: string) {
    setCheckedFields(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  const allChecked = diffItems.length > 0 && checkedFields.size === diffItems.length
  const noneChecked = checkedFields.size === 0 && !applyArt

  async function handleAccept() {
    setSaving(true); setError(null)
    try {
      const fields = [...checkedFields]
      await tagSuggestionsApi.accept(
        suggestion.id,
        fields.length < diffItems.length ? fields : undefined,
        applyArt,
      )
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      onDone()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to accept')
    } finally {
      setSaving(false)
    }
  }

  async function handleReject() {
    setSaving(true); setError(null)
    try {
      await tagSuggestionsApi.reject(suggestion.id)
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      onDone()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to reject')
    } finally {
      setSaving(false)
    }
  }

  const pct = Math.round(suggestion.confidence * 100)

  return (
    <div className="flex flex-col h-full">
      {/* Pane header */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border flex-shrink-0">
        {showBack && (
          <button onClick={onBack} className="text-xs text-text-muted hover:text-text-primary mr-1">
            ← Back
          </button>
        )}
        <span className="text-xs text-text-muted truncate flex-1">
          {track.title ?? track.relative_path.split('/').pop()}
        </span>
        <span className={`text-xs font-mono ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>{pct}% match</span>
        <button
          onClick={() => setCheckedFields(allChecked ? new Set() : new Set(diffItems.map(d => d.key)))}
          className="text-xs text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5"
        >
          {allChecked ? 'None' : 'All'}
        </button>
        {error && <span className="text-xs text-destructive">{error}</span>}
        <button
          onClick={handleReject}
          disabled={saving}
          className="text-xs text-text-muted hover:text-destructive border border-border rounded px-2 py-0.5 disabled:opacity-40"
        >
          Reject
        </button>
        <button
          onClick={handleAccept}
          disabled={saving || noneChecked}
          className="text-xs bg-accent text-bg-base rounded px-3 py-0.5 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {saving ? 'Saving…' : `Accept${checkedFields.size < diffItems.length ? ` (${checkedFields.size})` : ''}`}
        </button>
      </div>

      {/* Field diff table */}
      <div className="flex-1 overflow-y-auto">
        {/* Column header */}
        <div className="grid grid-cols-[1fr_1fr_1fr_1.5rem] gap-x-2 px-3 py-1 bg-bg-panel border-b border-border text-[10px] uppercase tracking-wider text-text-muted sticky top-0">
          <span>Field</span>
          <span>Current</span>
          <span>Suggested</span>
          <span />
        </div>
        {/* Art row — separate from field checkedFields */}
        <div
          className="grid grid-cols-[1fr_1fr_1fr_1.5rem] gap-x-2 px-3 py-1 border-b border-border-subtle items-center cursor-pointer select-none hover:bg-bg-row-hover"
          onClick={() => setApplyArt(prev => !prev)}
        >
          <span className="text-[11px] text-text-muted">Cover Art</span>
          <span className="text-xs text-text-secondary">
            {track.has_embedded_art ? 'embedded' : <em className="not-italic text-text-muted/40">—</em>}
          </span>
          <span className="text-xs">
            {suggestion.cover_art_url ? (
              <img src={suggestion.cover_art_url} alt="suggested art" className="w-8 h-8 object-cover rounded" />
            ) : (
              <em className="not-italic text-text-muted/40">—</em>
            )}
          </span>
          <span className="flex items-center justify-center">
            <Checkbox checked={applyArt} onChange={() => setApplyArt(prev => !prev)} />
          </span>
        </div>
        {diffItems.map(({ key, currentVal, suggestedVal, changed }) => (
          <div
            key={key}
            className={`grid grid-cols-[1fr_1fr_1fr_1.5rem] gap-x-2 px-3 py-1 border-b border-border-subtle items-center cursor-pointer select-none ${
              changed ? 'hover:bg-bg-row-hover' : 'opacity-50'
            }`}
            onClick={() => toggleField(key)}
          >
            <span className="text-[11px] text-text-muted truncate" title={key}>
              {FIELD_LABELS[key] ?? key}
            </span>
            <span className="text-xs text-text-secondary truncate font-mono">{currentVal || <em className="not-italic text-text-muted/40">—</em>}</span>
            <span className={`text-xs truncate font-mono ${changed ? 'text-text-primary' : 'text-text-secondary'}`}>
              {suggestedVal || <em className="not-italic text-text-muted/40">—</em>}
            </span>
            <span className="flex items-center justify-center">
              <Checkbox
                checked={checkedFields.has(key)}
                onChange={() => toggleField(key)}
              />
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

// ── ArtUpdateModal ─────────────────────────────────────────────────────────────
function ArtUpdateModal({
  trackIds,
  label,
  onClose,
}: {
  trackIds: number[]
  label: string
  onClose: () => void
}) {
  const [url, setUrl] = useState('')
  const [uploading, setUploading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [done, setDone] = useState(false)
  const [dragOver, setDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const uploadFile = useCallback(async (file: File) => {
    setUploading(true)
    setError(null)
    try {
      const ext = file.name.split('.').pop()?.toLowerCase() ?? 'bin'
      const safe = new File([file], `upload.${ext}`, { type: file.type })
      const form = new FormData()
      form.append('file', safe)
      const resp = await fetch('/api/v1/uploads/images', {
        method: 'POST',
        body: form,
        credentials: 'include',
      })
      if (!resp.ok) {
        const body = await resp.text()
        let msg = body
        try { msg = JSON.parse(body).error ?? body } catch { /* raw */ }
        throw new Error(msg)
      }
      const { url: uploaded } = await resp.json()
      setUrl(uploaded)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Upload failed')
    } finally {
      setUploading(false)
    }
  }, [])

  function handleDrop(e: React.DragEvent) {
    e.preventDefault()
    setDragOver(false)
    const file = e.dataTransfer.files[0]
    if (file) uploadFile(file)
  }

  async function handleEmbed() {
    if (!url.trim()) return
    setSaving(true); setError(null)
    try {
      for (const id of trackIds) {
        await artApi.embedFromUrl(id, url.trim())
      }
      setDone(true)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to embed art')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/60"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div className="bg-bg-panel border border-border rounded shadow-2xl w-[420px] flex flex-col">
        <div className="px-5 pt-4 pb-3 border-b border-border">
          <h2 className="text-sm font-semibold text-text-primary">Update art</h2>
        </div>
        <div className="px-5 py-4 flex flex-col gap-3">
          {done ? (
            <p className="text-xs text-green-400">
              Art embed job{trackIds.length > 1 ? 's' : ''} queued for <span className="font-medium">{label}</span>.
            </p>
          ) : (
            <>
              <p className="text-xs text-text-muted">
                {label}{trackIds.length > 1 ? ` (${trackIds.length} tracks)` : ''}
              </p>

              {/* Drop zone */}
              <div
                className={`flex flex-col items-center justify-center gap-2 border-2 border-dashed rounded p-4 cursor-pointer transition-colors
                  ${dragOver ? 'border-accent bg-accent/10' : 'border-border hover:border-accent/60'}`}
                onDragOver={e => { e.preventDefault(); setDragOver(true) }}
                onDragLeave={() => setDragOver(false)}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
              >
                {url ? (
                  <img
                    src={url}
                    alt="art preview"
                    className="w-20 h-20 object-cover rounded border border-border"
                    onError={e => (e.currentTarget.style.display = 'none')}
                  />
                ) : (
                  <span className="text-xs text-text-muted/60 select-none">
                    {uploading ? 'Uploading…' : 'Drop image here or click to browse'}
                  </span>
                )}
              </div>
              <input
                ref={fileInputRef}
                type="file"
                accept="image/jpeg,image/png,image/webp,image/gif"
                className="sr-only"
                onChange={e => { const f = e.target.files?.[0]; if (f) uploadFile(f); e.target.value = '' }}
              />

              {/* URL input */}
              <input
                type="text"
                placeholder="Or paste a cover art URL (https://…)"
                value={url}
                onChange={e => setUrl(e.target.value)}
                onKeyDown={e => { if (e.key === 'Enter') handleEmbed() }}
                className="text-xs bg-bg-base border border-border rounded px-3 py-1.5 text-text-primary placeholder:text-text-muted/50 focus:outline-none focus:border-accent"
              />
              {error && <p className="text-xs text-destructive">{error}</p>}
            </>
          )}
        </div>
        <div className="px-5 pb-4 pt-1 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="text-xs border border-border text-text-muted rounded px-3 py-1 hover:text-text-primary hover:border-accent"
          >
            {done ? 'Close' : 'Cancel'}
          </button>
          {!done && (
            <button
              onClick={handleEmbed}
              disabled={saving || uploading || !url.trim()}
              className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {saving ? 'Embedding…' : 'Embed'}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

// ── DeleteConfirmModal ─────────────────────────────────────────────────────────
function DeleteConfirmModal({
  label,
  submitting,
  onCancel,
  onConfirm,
}: {
  label: string
  submitting: boolean
  onCancel: () => void
  onConfirm: () => void
}) {
  return (
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/60"
      onClick={e => { if (e.target === e.currentTarget) onCancel() }}
    >
      <div className="bg-bg-panel border border-border rounded shadow-2xl w-[420px] flex flex-col">
        {/* Header */}
        <div className="px-5 pt-4 pb-3 border-b border-border">
          <h2 className="text-sm font-semibold text-text-primary">Schedule deletion</h2>
        </div>
        {/* Body */}
        <div className="px-5 py-4 flex flex-col gap-3">
          <p className="text-xs text-text-primary">
            The following will be scheduled for deletion from disk:
          </p>
          <div className="bg-bg-base border border-border rounded px-3 py-2 text-xs font-mono text-text-secondary">
            {label}
          </div>
          <p className="text-xs text-text-muted">
            Deletion runs after a <span className="text-text-primary font-medium">15-minute delay</span>.
            You can cancel it from the <span className="text-text-primary font-medium">Jobs</span> page before it runs.
          </p>
        </div>
        {/* Footer */}
        <div className="px-5 pb-4 pt-1 flex justify-end gap-2">
          <button
            onClick={onCancel}
            disabled={submitting}
            className="text-xs border border-border text-text-muted rounded px-3 py-1 hover:text-text-primary hover:border-accent disabled:opacity-40"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={submitting}
            className="text-xs bg-destructive text-white rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {submitting ? 'Scheduling…' : 'Schedule Deletion'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ── ContextMenu ────────────────────────────────────────────────────────────────
function ContextMenu({ x, y, items }: { x: number; y: number; items: MenuItem[] }) {
  const ref = React.useRef<HTMLDivElement>(null)
  const [pos, setPos] = React.useState({ x, y })

  React.useEffect(() => {
    const el = ref.current
    if (!el) return
    const rect = el.getBoundingClientRect()
    let nx = x
    let ny = y
    if (nx + rect.width > window.innerWidth)   nx = window.innerWidth  - rect.width  - 4
    if (ny + rect.height > window.innerHeight) ny = window.innerHeight - rect.height - 4
    if (nx < 4) nx = 4
    if (ny < 4) ny = 4
    setPos({ x: nx, y: ny })
  }, [x, y])

  return (
    <div
      ref={ref}
      style={{ position: 'fixed', left: pos.x, top: pos.y }}
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
