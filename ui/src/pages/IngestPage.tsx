import { useState, useEffect, useMemo, useRef } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { ImageUpload } from '../components/ImageUpload'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import {
  getStagedTracks,
  submitTrack,
  checkSupersede,
  type SupersedeMatchInfo,
} from '../api/ingest'
import { enqueueLookup, getPendingTags, setPendingTags, clearPendingTags } from '../api/tracks'
import { listLibraryProfiles } from '../api/libraryProfiles'
import { listSettings } from '../api/settings'
import type { Track } from '../types/track'
import type { TagSuggestion, AlternativeRelease } from '../types/tagSuggestion'
import type { LibraryProfile } from '../types/libraryProfile'

// ── Track fields ──────────────────────────────────────────────────────────────

interface TagField { key: string; label: string }

const ALBUM_FIELDS: TagField[] = [
  { key: 'albumartist',                label: 'Album Artist' },
  { key: 'albumartistsort',            label: 'Album Artist Sort' },
  { key: 'album',                      label: 'Album' },
  { key: 'date',                       label: 'Date' },
  { key: 'originalyear',              label: 'Original Year' },
  { key: 'originaldate',              label: 'Original Release Date' },
  { key: 'releasetype',                label: 'Release Type' },
  { key: 'releasestatus',              label: 'Release Status' },
  { key: 'releasecountry',             label: 'Release Country' },
  { key: 'totaltracks',               label: 'Total Tracks' },
  { key: 'totaldiscs',                label: 'Total Discs' },
  { key: 'label',                      label: 'Record Label' },
  { key: 'catalognumber',              label: 'Catalog #' },
  { key: 'barcode',                    label: 'Barcode' },
  { key: 'musicbrainz_albumartistid',  label: 'MB Artist ID' },
  { key: 'musicbrainz_releasegroupid', label: 'MB Release Group ID' },
  { key: 'musicbrainz_releaseid',      label: 'MB Release ID' },
]

const ALBUM_FIELD_KEYS = new Set(ALBUM_FIELDS.map(f => f.key))

const TRACK_FIELDS: TagField[] = [
  { key: 'title',                label: 'Title' },
  { key: 'artist',               label: 'Artist' },
  { key: 'tracknumber',          label: 'Track #' },
  { key: 'discnumber',           label: 'Disc #' },
  { key: 'album',                label: 'Album' },
  { key: 'albumartist',          label: 'Album Artist' },
  { key: 'date',                 label: 'Date' },
  { key: 'totaltracks',         label: 'Total Tracks' },
  { key: 'totaldiscs',          label: 'Total Discs' },
  { key: 'genre',                label: 'Genre' },
  { key: 'label',                label: 'Label' },
  { key: 'catalognumber',        label: 'Catalog #' },
  { key: 'musicbrainz_releaseid',      label: 'MB Release ID' },
  { key: 'musicbrainz_recordingid',    label: 'MB Recording ID' },
  { key: 'musicbrainz_albumartistid',  label: 'MB Artist ID' },
  { key: 'musicbrainz_releasegroupid', label: 'MB Release Group ID' },
]

const REQUIRED_FIELDS = new Set(['title', 'tracknumber', 'album', 'albumartist', 'date'])

// ── Helpers ───────────────────────────────────────────────────────────────────

function getTagValue(track: Track, key: string): string {
  const top = (track as unknown as Record<string, string | undefined>)[key]
  if (top != null) return top
  const v = (track.tags as Record<string, unknown>)[key]
  if (typeof v === 'string') return v
  if (v != null) return String(v)
  return ''
}

function getIngestFolder(relativePath: string): string {
  const stripped = relativePath.replace(/^ingest\//, '')
  const lastSlash = stripped.lastIndexOf('/')
  return lastSlash === -1 ? '(root)' : stripped.slice(0, lastSlash)
}

// ── Status helpers ────────────────────────────────────────────────────────────

type TrackStatus = 'ready' | 'review' | 'no-match'

function getTrackStatus(
  workingTags: Record<string, string> | null,
  suggestion: TagSuggestion | undefined,
): TrackStatus {
  const tags = workingTags ?? {}
  const hasAllRequired = [...REQUIRED_FIELDS].every(k => (tags[k] ?? '').trim() !== '')
  if (hasAllRequired) return 'ready'
  if (suggestion) return 'review'
  return 'no-match'
}

// ── Page ──────────────────────────────────────────────────────────────────────

export default function IngestPage() {
  const qc = useQueryClient()
  const [threshold, setThreshold] = useState(80)
  const [groupMode, setGroupMode] = useState<'album' | 'folder'>('album')
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)
  const [submitAlbum, setSubmitAlbum] = useState<string | null>(null)
  const [expandedTrackId, setExpandedTrackId] = useState<number | null>(null)
  const [albumArtUrls, setAlbumArtUrls] = useState<Record<string, string>>({})

  const { data: stagedTracks = [], isLoading: tracksLoading } = useQuery({
    queryKey: ['ingest-staged'],
    queryFn: getStagedTracks,
  })

  const { data: suggestions = [] } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
  })

  const { data: supersedeResults = [] } = useQuery({
    queryKey: ['ingest-supersede', stagedTracks.map(t => t.id)],
    queryFn: () =>
      stagedTracks.length > 0
        ? checkSupersede(stagedTracks.map(t => t.id))
        : Promise.resolve([]),
    enabled: stagedTracks.length > 0,
  })

  const supersedeByTrack: Record<number, SupersedeMatchInfo> = {}
  for (const r of supersedeResults) {
    if (r.match) supersedeByTrack[r.track_id] = r.match
  }

  const batchAccept = useMutation({
    mutationFn: () => tagSuggestionsApi.batchAccept(threshold / 100),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
    },
  })

  const lookupMutation = useMutation({
    mutationFn: (trackId: number) => enqueueLookup(trackId),
  })

  const suggestionsByTrack: Record<number, TagSuggestion> = {}
  for (const s of suggestions) {
    const existing = suggestionsByTrack[s.track_id]
    if (!existing || s.confidence > existing.confidence) {
      suggestionsByTrack[s.track_id] = s
    }
  }

  const groups: Record<string, Track[]> = {}
  for (const track of stagedTracks) {
    const key = groupMode === 'folder'
      ? getIngestFolder(track.relative_path)
      : (track.album ?? 'Unknown Album')
    if (!groups[key]) groups[key] = []
    groups[key].push(track)
  }
  const sortedAlbums = Object.keys(groups).sort()

  if (tracksLoading) {
    return (
      <>
        <TopNav />
        <div className="p-4 text-text-muted text-sm">Loading…</div>
      </>
    )
  }

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Toolbar */}
        <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-bg-surface flex-shrink-0">
          <span className="text-xs text-text-muted">
            {stagedTracks.length === 0
              ? 'No staged tracks'
              : `${stagedTracks.length} staged track${stagedTracks.length !== 1 ? 's' : ''}`}
          </span>
          <div className="flex items-center border border-border rounded overflow-hidden text-[11px] font-mono">
            <button
              onClick={() => setGroupMode('album')}
              className={`px-2 py-0.5 ${groupMode === 'album' ? 'bg-accent text-bg-base' : 'text-text-muted hover:text-text-primary'}`}
            >
              Album
            </button>
            <button
              onClick={() => setGroupMode('folder')}
              className={`px-2 py-0.5 border-l border-border ${groupMode === 'folder' ? 'bg-accent text-bg-base' : 'text-text-muted hover:text-text-primary'}`}
            >
              Folder
            </button>
          </div>
          <div className="flex items-center gap-2 ml-auto">
            <span className="text-xs text-text-muted">Accept all ≥</span>
            <input
              type="number" min={1} max={100} value={threshold}
              onChange={e => setThreshold(Number(e.target.value))}
              className="w-14 bg-bg-base border border-border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent"
            />
            <span className="text-xs text-text-muted">%</span>
            <button
              onClick={() => batchAccept.mutate()}
              disabled={batchAccept.isPending || suggestions.length === 0}
              className="text-xs bg-accent text-bg-base rounded px-3 py-1 hover:opacity-90 disabled:opacity-50"
            >
              {batchAccept.isPending ? 'Accepting…' : `Accept ≥${threshold}%`}
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-6">
          {sortedAlbums.length === 0 ? (
            <p className="text-center text-text-muted text-sm pt-12">No staged tracks</p>
          ) : (
            sortedAlbums.map(albumKey => (
              <AlbumGroup
                key={albumKey}
                albumKey={albumKey}
                tracks={groups[albumKey]}
                suggestionsByTrack={suggestionsByTrack}
                supersedeByTrack={supersedeByTrack}
                expandedTrackId={expandedTrackId}
                onToggleExpand={id => setExpandedTrackId(prev => prev === id ? null : id)}
                onSearch={t => setSearchTrack(t)}
                onLookup={id => lookupMutation.mutate(id)}
                onSubmitAlbum={key => setSubmitAlbum(key)}
                lookupPending={lookupMutation.isPending ? lookupMutation.variables ?? null : null}
                presetArtUrl={albumArtUrls[albumKey] ?? ''}
                onArtChange={url => setAlbumArtUrls(prev => ({ ...prev, [albumKey]: url }))}
              />
            ))
          )}
        </div>
      </div>

      {submitAlbum != null && groups[submitAlbum] && (
        <SubmitDialog
          albumKey={submitAlbum}
          tracks={groups[submitAlbum]}
          suggestionsByTrack={suggestionsByTrack}
          supersedeByTrack={supersedeByTrack}
          onClose={() => setSubmitAlbum(null)}
          onSubmitted={() => {
            setSubmitAlbum(null)
            qc.invalidateQueries({ queryKey: ['ingest-staged'] })
          }}
          presetArtUrl={albumArtUrls[submitAlbum] ?? ''}
        />
      )}

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

// ── AlbumGroup ────────────────────────────────────────────────────────────────

function AlbumGroup({
  albumKey,
  tracks,
  suggestionsByTrack,
  supersedeByTrack,
  expandedTrackId,
  onToggleExpand,
  onSearch,
  onLookup,
  onSubmitAlbum,
  lookupPending,
  presetArtUrl,
  onArtChange,
}: {
  albumKey: string
  tracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  supersedeByTrack: Record<number, SupersedeMatchInfo>
  expandedTrackId: number | null
  onToggleExpand: (id: number) => void
  onSearch: (t: Track) => void
  onLookup: (id: number) => void
  onSubmitAlbum: (key: string) => void
  lookupPending: number | null
  presetArtUrl: string
  onArtChange: (url: string) => void
}) {
  const firstTrack = tracks[0]
  const firstSuggestion = suggestionsByTrack[firstTrack.id]
  const coverArtUrl = firstSuggestion?.cover_art_url
  const displayArtUrl = presetArtUrl || coverArtUrl
  const hasEmbeddedArt = tracks.some(t => t.has_embedded_art)
  const formatExt = firstTrack.relative_path.split('.').pop()?.toUpperCase() ?? '?'
  const supersedeCount = tracks.filter(t => supersedeByTrack[t.id]).length

  const [editingAlbum, setEditingAlbum] = useState(false)
  const [showArtUpload, setShowArtUpload] = useState(false)
  const [selectedAltIdx, setSelectedAltIdx] = useState<number | null>(null)

  // Album-level alternatives driven by the first track's best suggestion
  const albumAlternatives = firstSuggestion?.alternatives ?? []
  const primaryAlbumLabel = (firstSuggestion?.suggested_tags as Record<string, string> | undefined)?.album
    ?? firstTrack.album ?? 'Primary suggestion'

  return (
    <div className="border border-border rounded bg-bg-panel">
      {/* Album header */}
      <div className="flex items-center gap-3 px-3 py-2 border-b border-border">
        {displayArtUrl && (
          <img
            src={displayArtUrl} alt=""
            className="w-14 h-14 object-cover rounded border border-border flex-shrink-0"
            onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
          />
        )}
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className="text-text-primary text-sm font-semibold truncate">{albumKey}</span>
          <span className="text-xs text-text-muted shrink-0">
            {tracks.length} track{tracks.length !== 1 ? 's' : ''}
          </span>
          <span className="text-[10px] font-mono uppercase text-text-muted border border-border rounded px-1">
            {formatExt}
          </span>
          {supersedeCount > 0 && (
            <span className="text-[10px] font-mono uppercase text-sky-400 border border-sky-400/40 rounded px-1 shrink-0">
              {supersedeCount} replac{supersedeCount !== 1 ? 'e' : 'es'} existing
            </span>
          )}
          {hasEmbeddedArt && !displayArtUrl && (
            <span className="text-[10px] font-mono uppercase text-emerald-400 border border-emerald-400/40 rounded px-1 shrink-0">
              Embedded art
            </span>
          )}
          {albumAlternatives.length > 0 && (
            <select
              value={selectedAltIdx === null ? '' : String(selectedAltIdx)}
              onChange={e => setSelectedAltIdx(e.target.value === '' ? null : Number(e.target.value))}
              className="text-[11px] font-mono bg-bg-base border border-border rounded px-2 py-0.5 text-text-primary focus:outline-none focus:border-accent shrink-0 max-w-[220px] truncate"
              title="Switch album-level alternative (updates suggestion bars)"
            >
              <option value="">{primaryAlbumLabel}</option>
              {albumAlternatives.map((alt, i) => {
                const name = alt.suggested_tags.album ?? alt.mb_release_id
                const date = alt.suggested_tags.date ? ` (${alt.suggested_tags.date})` : ''
                const artist = alt.suggested_tags.albumartist ? ` · ${alt.suggested_tags.albumartist}` : ''
                return <option key={i} value={String(i)}>{name}{date}{artist}</option>
              })}
            </select>
          )}
        </div>
        <button
          onClick={() => setShowArtUpload(v => !v)}
          className={`text-xs border rounded px-3 py-1 font-medium shrink-0 ${showArtUpload ? 'border-accent text-accent' : 'border-border text-text-muted hover:text-text-primary'}`}
        >
          {displayArtUrl ? 'Change Art' : hasEmbeddedArt ? 'Replace Art' : 'Add Art'}
        </button>
        <button
          onClick={() => setEditingAlbum(v => !v)}
          className={`text-xs border rounded px-3 py-1 font-medium shrink-0 ${editingAlbum ? 'border-accent text-accent' : 'border-border text-text-muted hover:text-text-primary'}`}
        >
          Edit Album
        </button>
        <button
          onClick={() => onSubmitAlbum(albumKey)}
          className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 shrink-0"
        >
          Import Album
        </button>
      </div>

      {/* Inline art upload */}
      {showArtUpload && (
        <div className="border-b border-border bg-bg-base px-3 py-2 flex items-center gap-3">
          <span className="text-[10px] uppercase tracking-wide text-text-muted font-mono shrink-0">Cover Art</span>
          <div className="flex-1">
            <ImageUpload
              value={presetArtUrl}
              onChange={url => { onArtChange(url); setShowArtUpload(false) }}
            />
          </div>
          {presetArtUrl && (
            <button type="button" onClick={() => { onArtChange(''); setShowArtUpload(false) }}
              className="text-xs text-text-muted border border-border rounded px-2 py-0.5 hover:text-destructive shrink-0">
              Remove
            </button>
          )}
          <button type="button" onClick={() => setShowArtUpload(false)}
            className="text-xs text-text-muted border border-border rounded px-2 py-0.5 hover:text-text-primary shrink-0">
            Close
          </button>
        </div>
      )}

      {/* Album bulk-edit panel */}
      {editingAlbum && (
        <AlbumEditPanel
          tracks={tracks}
          suggestions={suggestionsByTrack}
          selectedAltIdx={selectedAltIdx}
          onClose={() => setEditingAlbum(false)}
        />
      )}

      {/* Track rows */}
      <div className="flex flex-col divide-y divide-border">
        {tracks.map(track => (
          <TrackRow
            key={track.id}
            track={track}
            suggestion={suggestionsByTrack[track.id]}
            supersede={supersedeByTrack[track.id]}
            albumAltIdx={selectedAltIdx}
            isExpanded={expandedTrackId === track.id}
            onToggleExpand={() => onToggleExpand(track.id)}
            onSearch={() => onSearch(track)}
            onLookup={() => onLookup(track.id)}
            lookupPending={lookupPending === track.id}
          />
        ))}
      </div>
    </div>
  )
}

// ── TrackRow ──────────────────────────────────────────────────────────────────

function TrackRow({
  track,
  suggestion,
  supersede,
  albumAltIdx,
  isExpanded,
  onToggleExpand,
  onSearch,
  onLookup,
  lookupPending,
}: {
  track: Track
  suggestion: TagSuggestion | undefined
  supersede: SupersedeMatchInfo | undefined
  albumAltIdx: number | null
  isExpanded: boolean
  onToggleExpand: () => void
  onSearch: () => void
  onLookup: () => void
  lookupPending: boolean
}) {
  // Working copy state — loaded from backend, saved on blur
  const [workingTags, setWorkingTags] = useState<Record<string, string> | null>(null)
  const [saving, setSaving] = useState(false)
  const [trackAltIdx, setTrackAltIdx] = useState<number | null>(null)
  const [expandedSupersede, setExpandedSupersede] = useState(false)
  const loadedRef = useRef(false)

  // Effective alternative: track-level override takes priority over album-level
  const effectiveAltIdx = trackAltIdx ?? albumAltIdx
  const effectiveAlt: AlternativeRelease | undefined =
    suggestion && effectiveAltIdx !== null
      ? suggestion.alternatives?.[effectiveAltIdx]
      : undefined

  // Load working copy on first expand
  useEffect(() => {
    if (!isExpanded || loadedRef.current) return
    loadedRef.current = true
    getPendingTags(track.id).then(tags => {
      if (Object.keys(tags).length > 0) {
        setWorkingTags(tags)
        // Auto-select the alternative whose release ID matches the working copy
        const releaseId = tags.musicbrainz_releaseid
        if (releaseId && suggestion?.alternatives) {
          const matchIdx = suggestion.alternatives.findIndex(alt => alt.mb_release_id === releaseId)
          if (matchIdx >= 0) setTrackAltIdx(matchIdx)
        }
      } else {
        // Seed from current track tags
        const seed: Record<string, string> = {}
        for (const { key } of TRACK_FIELDS) {
          const v = getTagValue(track, key)
          if (v) seed[key] = v
        }
        setWorkingTags(seed)
      }
    }).catch(() => {
      const seed: Record<string, string> = {}
      for (const { key } of TRACK_FIELDS) {
        const v = getTagValue(track, key)
        if (v) seed[key] = v
      }
      setWorkingTags(seed)
    })
  }, [isExpanded, track, suggestion])

  const status = getTrackStatus(workingTags, suggestion)
  const pct = suggestion ? Math.round(suggestion.confidence * 100) : null

  // Title for collapsed summary
  const displayTitle = workingTags?.title || getTagValue(track, 'title') || track.relative_path.split('/').pop() || ''
  const displayArtist = workingTags?.artist || getTagValue(track, 'artist') || ''
  const displayTrackNum = workingTags?.tracknumber || getTagValue(track, 'tracknumber') || ''

  async function handleBlurSave(updatedTags: Record<string, string>) {
    setSaving(true)
    try {
      await setPendingTags(track.id, updatedTags)
    } finally {
      setSaving(false)
    }
  }

  async function handleApplySuggested() {
    const src = effectiveAlt?.suggested_tags ?? (suggestion?.suggested_tags as Record<string, string> | undefined)
    if (!src) return
    const merged = { ...(workingTags ?? {}), ...src }
    setWorkingTags(merged)
    // Lock the dropdown to whichever alternative was just applied
    if (effectiveAltIdx !== null) setTrackAltIdx(effectiveAltIdx)
    await setPendingTags(track.id, merged)
  }

  async function handleReset() {
    await clearPendingTags(track.id)
    loadedRef.current = false
    const seed: Record<string, string> = {}
    for (const { key } of TRACK_FIELDS) {
      const v = getTagValue(track, key)
      if (v) seed[key] = v
    }
    setWorkingTags(seed)
  }

  function updateField(key: string, value: string) {
    const updated = { ...(workingTags ?? {}), [key]: value }
    setWorkingTags(updated)
    return updated
  }

  // Suggestion bar styling
  const sugConf = suggestion?.confidence ?? 0
  const sugBarClass =
    !suggestion
      ? 'border-border bg-bg-surface'
      : sugConf >= 0.8
        ? 'border-accent/40 bg-accent/5'
        : 'border-amber-400/35 bg-amber-400/5'

  return (
    <div className={`flex flex-col ${isExpanded ? 'bg-bg-base' : ''}`}>
      {/* Collapsed header — always visible */}
      <div
        className={`grid grid-cols-[20px_1fr_auto_auto_auto_12px] gap-x-2 items-center px-3 py-1.5 cursor-pointer select-none ${isExpanded ? 'bg-bg-surface border-b border-border' : 'hover:bg-bg-row-hover'}`}
        onClick={onToggleExpand}
      >
        <span className="text-text-muted font-mono text-[10px] text-right">{track.tracknumber ?? '—'}</span>
        <div className="min-w-0">
          <div className="text-text-primary text-xs truncate">{displayTitle}</div>
          <div className="text-text-muted font-mono text-[10px] truncate">
            {track.relative_path.split('/').pop()}
          </div>
        </div>
        {/* Confidence badge */}
        <span className={`text-[10px] font-mono shrink-0 ${pct == null ? 'text-text-muted' : pct >= 80 ? 'text-accent' : 'text-amber-400'}`}>
          {pct != null ? `${pct}%` : '—'}
        </span>
        {/* Status pill */}
        <span className={`text-[9px] font-mono uppercase tracking-wide border rounded px-1.5 py-0.5 shrink-0 ${
          status === 'ready' ? 'text-green-400 border-green-400/30' :
          status === 'review' ? 'text-amber-400 border-amber-400/30' :
          'text-text-muted border-border'
        }`}>
          {status === 'no-match' ? 'no match' : status}
        </span>
        {/* Tag summary */}
        <span className="text-[10px] text-text-muted truncate max-w-[160px] shrink-0">
          {[displayTitle, displayArtist, displayTrackNum].filter(Boolean).join(' · ')}
        </span>
        {/* Chevron */}
        <span className={`text-text-muted text-[10px] transition-transform ${isExpanded ? 'rotate-90' : ''}`}>›</span>
      </div>

      {/* Supersede badge (collapsed view) */}
      {supersede && !isExpanded && (
        <div className="px-3 pb-1">
          <button
            onClick={e => { e.stopPropagation(); setExpandedSupersede(v => !v) }}
            className={`text-[9px] font-mono uppercase rounded px-1 border ${
              supersede.profile_match
                ? 'text-sky-400 border-sky-400/40 hover:border-sky-400'
                : 'text-amber-400 border-amber-400/40 hover:border-amber-400'
            }`}
          >
            {supersede.profile_match ? 'Replaces existing' : '⚠ Replaces existing'}
          </button>
        </div>
      )}
      {expandedSupersede && supersede && !isExpanded && (
        <div className="px-3 pb-2">
          <SupersedeDetailRow supersede={supersede} />
        </div>
      )}

      {/* Expanded panel */}
      {isExpanded && workingTags !== null && (
        <div className="flex flex-col">
          {/* Supersede row */}
          {supersede && (
            <div className="px-3 pt-2">
              <SupersedeDetailRow supersede={supersede} />
            </div>
          )}

          {/* Suggestion bar */}
          <div className={`mx-3 mt-2 flex items-center gap-2 px-3 py-2 rounded border text-[10px] ${sugBarClass}`}>
            {suggestion ? (
              <>
                <span className="text-text-muted uppercase tracking-wide font-mono shrink-0">{suggestion.source}</span>
                <span className={`font-mono shrink-0 ${sugConf >= 0.8 ? 'text-accent' : 'text-amber-400'}`}>
                  {Math.round(sugConf * 100)}%
                </span>
                <span className="text-text-secondary flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                  {effectiveAlt
                    ? `${effectiveAlt.suggested_tags.album ?? '?'} · ${effectiveAlt.suggested_tags.albumartist ?? ''} · ${effectiveAlt.suggested_tags.date ?? ''}`
                    : `${(suggestion.suggested_tags as Record<string,string>).album ?? '?'} · ${(suggestion.suggested_tags as Record<string,string>).albumartist ?? ''} · ${(suggestion.suggested_tags as Record<string,string>).date ?? ''}`
                  }
                  {((effectiveAlt?.suggested_tags ?? suggestion.suggested_tags as Record<string,string>).tracknumber) &&
                    ` · track ${(effectiveAlt?.suggested_tags ?? suggestion.suggested_tags as Record<string,string>).tracknumber}`
                  }
                </span>
                {/* Track-level alternatives dropdown */}
                {(suggestion.alternatives?.length ?? 0) > 0 && (
                  <select
                    value={trackAltIdx === null ? '' : String(trackAltIdx)}
                    onChange={e => setTrackAltIdx(e.target.value === '' ? null : Number(e.target.value))}
                    onClick={e => e.stopPropagation()}
                    className="text-[10px] font-mono bg-bg-base border border-border rounded px-1.5 py-0.5 text-text-primary focus:outline-none focus:border-accent max-w-[150px]"
                    title="Select alternate release for this track only"
                  >
                    <option value="">{(suggestion.suggested_tags as Record<string,string>).album ?? 'Primary'}</option>
                    {suggestion.alternatives!.map((alt, i) => (
                      <option key={i} value={String(i)}>
                        {alt.suggested_tags.album ?? alt.mb_release_id}{alt.suggested_tags.date ? ` (${alt.suggested_tags.date})` : ''}
                      </option>
                    ))}
                  </select>
                )}
              </>
            ) : (
              <span className="text-text-muted flex-1">No lookup result — use Search or Lookup to find a match</span>
            )}
            <div className="flex items-center gap-1 shrink-0">
              {suggestion && (
                <button
                  onClick={e => { e.stopPropagation(); handleApplySuggested() }}
                  className="border border-accent text-accent rounded px-2 py-0.5 font-mono hover:bg-accent/10"
                >
                  Apply Suggested
                </button>
              )}
              <button
                onClick={e => { e.stopPropagation(); onSearch() }}
                className="border border-border text-text-muted rounded px-2 py-0.5 font-mono hover:text-text-primary"
              >
                Search
              </button>
              <button
                onClick={e => { e.stopPropagation(); onLookup() }}
                disabled={lookupPending}
                className="border border-border text-text-muted rounded px-2 py-0.5 font-mono hover:text-text-primary disabled:opacity-50"
              >
                {lookupPending ? 'Queued…' : 'Lookup'}
              </button>
            </div>
          </div>

          {/* Edit fields */}
          <WorkingTagsEditor
            trackId={track.id}
            workingTags={workingTags}
            onUpdateField={updateField}
            onBlurSave={handleBlurSave}
            onReset={handleReset}
            saving={saving}
          />
        </div>
      )}
    </div>
  )
}

// ── WorkingTagsEditor ─────────────────────────────────────────────────────────

function WorkingTagsEditor({
  trackId: _trackId,
  workingTags,
  onUpdateField,
  onBlurSave,
  onReset,
  saving,
}: {
  trackId: number
  workingTags: Record<string, string>
  onUpdateField: (key: string, value: string) => Record<string, string>
  onBlurSave: (tags: Record<string, string>) => void
  onReset: () => void
  saving: boolean
}) {
  return (
    <div className="mx-3 mt-2 mb-3 border border-border rounded bg-bg-base">
      <div className="grid grid-cols-2 divide-x divide-border-subtle">
        {TRACK_FIELDS.map(({ key, label }) => {
          const value = workingTags[key] ?? ''
          const isMissing = REQUIRED_FIELDS.has(key) && value.trim() === ''
          return (
            <div
              key={key}
              className={`grid grid-cols-[6.5rem_1fr] gap-x-2 px-3 py-1 border-b border-border-subtle items-center ${isMissing ? 'bg-amber-400/5' : ''}`}
            >
              <span className={`text-[10px] font-mono truncate ${isMissing ? 'text-amber-400' : 'text-text-muted'}`}>
                {label}
              </span>
              <input
                type="text"
                value={value}
                placeholder="—"
                onChange={e => onUpdateField(key, e.target.value)}
                onBlur={e => {
                  const updated = onUpdateField(key, e.target.value)
                  onBlurSave(updated)
                }}
                className={`bg-transparent border-b py-0.5 font-mono text-[11px] focus:outline-none placeholder:text-text-muted/30 w-full ${
                  isMissing
                    ? 'border-amber-400/40 text-amber-400 focus:border-amber-400'
                    : 'border-transparent text-text-primary hover:border-border focus:border-accent'
                }`}
              />
            </div>
          )
        })}
      </div>
      <div className="flex items-center gap-2 px-3 py-1.5 border-t border-border">
        <span className="text-[10px] text-text-muted flex-1">
          {saving ? 'Saving…' : 'Working copy — edits saved automatically'}
        </span>
        <button
          onClick={onReset}
          className="text-[10px] border border-destructive/30 text-destructive rounded px-2 py-0.5 font-mono hover:border-destructive"
        >
          Reset
        </button>
      </div>
    </div>
  )
}

// ── SupersedeDetailRow ────────────────────────────────────────────────────────

function SupersedeDetailRow({ supersede }: { supersede: SupersedeMatchInfo }) {
  const fmtQuality = (fmt: string, sr: number | null, bd: number | null, br: number | null) => {
    const fmtStr = fmt.toUpperCase()
    const khz = sr != null ? `${(sr / 1000).toFixed(sr % 1000 === 0 ? 0 : 1)}kHz` : null
    if (bd != null) {
      return [fmtStr, khz, `${bd}-bit`].filter(Boolean).join(' · ')
    }
    if (khz && br) return `${fmtStr} · ${khz} / ${br}k`
    if (br) return `${fmtStr} · ${br}k`
    if (khz) return `${fmtStr} · ${khz}`
    return fmtStr
  }

  return (
    <div className="rounded border border-sky-400/20 bg-sky-400/5 px-3 py-2 text-[11px] flex flex-col gap-1">
      <div className="flex items-center gap-2 text-text-muted">
        <span className="font-mono text-[10px] uppercase tracking-wide text-sky-400">Replaces existing</span>
        <span className="text-[10px] text-text-muted/60">via {supersede.identity_method.replace('_', ' ')}</span>
      </div>
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <div className="text-text-muted text-[10px] uppercase tracking-wide mb-0.5">Current</div>
          <div className="font-mono text-text-secondary">
            {fmtQuality(supersede.active_track_format, supersede.active_track_sample_rate, supersede.active_track_bit_depth, supersede.active_track_bitrate)}
          </div>
        </div>
        <span className="text-text-muted">→</span>
        <div className="flex-1">
          {supersede.profile_match ? (
            <>
              <div className="text-text-muted text-[10px] uppercase tracking-wide mb-0.5">Moves to</div>
              <div className="font-mono text-sky-400">
                {supersede.profile_match.derived_dir_name}
                <span className="text-text-muted ml-1">({supersede.profile_match.profile_name})</span>
              </div>
            </>
          ) : (
            <>
              <div className="text-text-muted text-[10px] uppercase tracking-wide mb-0.5">Profile</div>
              <div className="font-mono text-amber-400">No matching profile — resolve in Import dialog</div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

// ── AlbumEditPanel ────────────────────────────────────────────────────────────

function AlbumEditPanel({
  tracks,
  suggestions,
  selectedAltIdx,
  onClose,
}: {
  tracks: Track[]
  suggestions: Record<number, TagSuggestion>
  selectedAltIdx: number | null
  onClose: () => void
}) {
  const currentValues = useMemo<Record<string, string>>(() => {
    const result: Record<string, string> = {}
    for (const { key } of ALBUM_FIELDS) {
      const vals = tracks.map(t => getTagValue(t, key)).filter(v => v !== '')
      const unique = [...new Set(vals)]
      result[key] = unique.length === 1 ? unique[0] : unique.length > 1 ? '(mixed)' : ''
    }
    return result
  }, [tracks])

  const [fields, setFields] = useState<Record<string, string>>(
    () => Object.fromEntries(ALBUM_FIELDS.map(f => [f.key, '']))
  )
  const [saving, setSaving] = useState(false)
  const [savedCount, setSavedCount] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Populate form from album-level suggestion (or selected alternative)
  function handleApplySuggested() {
    const firstSug = Object.values(suggestions).find(s => s)
    if (!firstSug) return
    const src = selectedAltIdx !== null
      ? firstSug.alternatives?.[selectedAltIdx]?.suggested_tags
      : (firstSug.suggested_tags as Record<string, string>)
    if (!src) return
    const filled: Record<string, string> = {}
    for (const { key } of ALBUM_FIELDS) {
      if (ALBUM_FIELD_KEYS.has(key) && src[key]) filled[key] = src[key]
    }
    setFields(prev => ({ ...prev, ...filled }))
  }

  async function handleApplyToAll() {
    // Merge new override values with consensus current values (skip "(mixed)" and blank)
    const albumTags: Record<string, string> = {}
    for (const { key } of ALBUM_FIELDS) {
      const newVal = fields[key].trim()
      const cur = currentValues[key]
      const val = newVal !== '' ? newVal : (cur !== '' && cur !== '(mixed)' ? cur : '')
      if (val !== '') albumTags[key] = val
    }
    if (Object.keys(albumTags).length === 0) return

    setSaving(true)
    setError(null)
    setSavedCount(null)
    let count = 0
    const errors: string[] = []
    for (const track of tracks) {
      try {
        // Read current pending_tags for this track, merge album fields in
        const existing = await getPendingTags(track.id)
        // Seed from track tags if no pending_tags
        const base: Record<string, string> = Object.keys(existing).length > 0 ? existing : {}
        if (Object.keys(base).length === 0) {
          for (const { key } of TRACK_FIELDS) {
            const v = getTagValue(track, key)
            if (v) base[key] = v
          }
        }
        const merged = { ...base, ...albumTags }
        await setPendingTags(track.id, merged)
        count++
      } catch (e) {
        errors.push(e instanceof Error ? e.message : 'unknown error')
      }
    }
    setSaving(false)
    setSavedCount(count)
    if (errors.length > 0) setError(`${errors.length} failed: ${errors[0]}`)
  }

  const pushCount = ALBUM_FIELDS.filter(({ key }) => {
    const newVal = fields[key].trim()
    const cur = currentValues[key]
    return newVal !== '' || (cur !== '' && cur !== '(mixed)')
  }).length

  return (
    <div className="border-b border-border bg-bg-base text-xs">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-bg-panel">
        <span className="text-[11px] text-text-muted font-mono">
          Album Tags — applies to all {tracks.length} tracks
        </span>
        <span className="flex-1" />
        {savedCount != null && (
          <span className="text-[11px] text-green-400">Applied to {savedCount} tracks</span>
        )}
        {error && <span className="text-[11px] text-destructive">{error}</span>}
        <button type="button" onClick={handleApplySuggested}
          className="text-[11px] border border-accent text-accent rounded px-2 py-0.5 font-mono hover:bg-accent/10">
          Apply Suggested
        </button>
        <button type="button" onClick={onClose}
          className="text-[11px] text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5">
          Close
        </button>
        <button type="button" onClick={handleApplyToAll} disabled={saving || pushCount === 0}
          className="text-[11px] bg-accent text-bg-base rounded px-3 py-0.5 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed">
          {saving ? 'Applying…' : `Apply to All (${pushCount})`}
        </button>
      </div>
      {ALBUM_FIELDS.map(({ key, label }) => {
        const current = currentValues[key]
        const newVal = fields[key]
        const hasNew = newVal.trim() !== ''
        return (
          <div key={key} className="grid grid-cols-[7rem_1fr_1fr] gap-x-2 px-3 py-0.5 border-b border-border-subtle items-center">
            <span className="text-[11px] text-text-muted font-mono truncate" title={label}>{label}</span>
            <span className={`text-[11px] font-mono truncate ${hasNew ? 'text-text-muted line-through' : 'text-text-secondary'}`}>
              {current || <em className="not-italic text-text-muted/40">—</em>}
            </span>
            <input
              type="text" value={newVal} placeholder="(unchanged)"
              onChange={e => { setSavedCount(null); setFields(prev => ({ ...prev, [key]: e.target.value })) }}
              className={`bg-transparent border-b py-0.5 font-mono text-[11px] focus:outline-none placeholder:text-text-muted/30 w-full ${
                hasNew ? 'border-accent text-text-primary' : 'border-transparent text-text-secondary hover:border-border focus:border-accent'
              }`}
            />
          </div>
        )
      })}
    </div>
  )
}

// ── SubmitDialog ──────────────────────────────────────────────────────────────

type SupersedeResolution = 'supersede' | 'skip' | 'discard'

function SubmitDialog({
  albumKey,
  tracks,
  suggestionsByTrack,
  supersedeByTrack,
  onClose,
  onSubmitted,
  presetArtUrl,
}: {
  albumKey: string
  tracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  supersedeByTrack: Record<number, SupersedeMatchInfo>
  onClose: () => void
  onSubmitted: () => void
  presetArtUrl: string
}) {
  const qc = useQueryClient()
  const libraryId = tracks[0].library_id

  const { data: profiles = [] } = useQuery({
    queryKey: ['library-profiles', libraryId],
    queryFn: () => listLibraryProfiles(libraryId),
  })

  const { data: settings = [] } = useQuery({
    queryKey: ['settings'],
    queryFn: listSettings,
  })
  const folderArtFilename = settings.find(s => s.key === 'folder_art_filename')?.value ?? ''

  const firstTrack = tracks[0]
  const sampleRate = firstTrack.sample_rate ?? null

  const [selectedProfiles, setSelectedProfiles] = useState<Set<number>>(new Set())
  const [trackWorkingTags, setTrackWorkingTags] = useState<Record<number, Record<string, string>>>({})
  const [loadingTags, setLoadingTags] = useState(true)

  // Load all working copies for the preview
  useEffect(() => {
    Promise.all(
      tracks.map(async t => {
        const tags = await getPendingTags(t.id).catch(() => ({}))
        return [t.id, tags] as const
      })
    ).then(entries => {
      setTrackWorkingTags(Object.fromEntries(entries))
      setLoadingTags(false)
    })
  }, [tracks.map(t => t.id).join(',')])

  useEffect(() => {
    if (profiles.length === 0) return
    const defaults = new Set(
      profiles
        .filter((p: LibraryProfile) =>
          p.include_on_submit &&
          (p.auto_include_above_hz == null || (sampleRate != null && sampleRate >= p.auto_include_above_hz))
        )
        .map((p: LibraryProfile) => p.id)
    )
    setSelectedProfiles(defaults)
  }, [profiles])

  const [queued, setQueued] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [uploadedArtUrl, setUploadedArtUrl] = useState<string>(presetArtUrl)

  const [supersedeResolutions, setSupersedeResolutions] = useState<Record<number, SupersedeResolution>>({})

  useEffect(() => {
    const initial: Record<number, SupersedeResolution> = {}
    for (const track of tracks) {
      const s = supersedeByTrack[track.id]
      if (s) initial[track.id] = 'supersede'
    }
    setSupersedeResolutions(initial)
  }, [tracks.map(t => t.id).join(','), Object.keys(supersedeByTrack).join(',')])

  const supersedeTrackIds = tracks.filter(t => supersedeByTrack[t.id])
  const unresolvedWarnings = supersedeTrackIds.filter(t => {
    const s = supersedeByTrack[t.id]
    if (!s || s.profile_match) return false
    const res = supersedeResolutions[t.id]
    return !res || res === 'supersede'
  })
  const canImport = unresolvedWarnings.length === 0

  // Track readiness from loaded working copies
  const trackStatuses = tracks.map(t => {
    const wt = trackWorkingTags[t.id]
    const tags = wt && Object.keys(wt).length > 0 ? wt : null
    return { track: t, status: getTrackStatus(tags, suggestionsByTrack[t.id]) }
  })
  const unreadyTracks = trackStatuses.filter(ts => ts.status !== 'ready')

  function toggleProfile(id: number) {
    setSelectedProfiles(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }

  const suggestion = suggestionsByTrack[firstTrack.id]
  const suggestedArtUrl = suggestion?.cover_art_url
  const albumHasEmbeddedArt = tracks.some(t => t.has_embedded_art)

  type ArtMode = 'use' | 'keep_embedded' | 'skip'
  const defaultArtMode: ArtMode =
    !uploadedArtUrl && !suggestedArtUrl && albumHasEmbeddedArt ? 'keep_embedded' : 'use'
  const [artMode, setArtMode] = useState<ArtMode>(defaultArtMode)

  const selectedArtUrl: string | undefined =
    artMode === 'skip' || artMode === 'keep_embedded'
      ? undefined
      : (uploadedArtUrl || suggestedArtUrl || undefined)

  const writeFolderArt =
    folderArtFilename !== '' && (selectedArtUrl != null || artMode === 'keep_embedded')

  async function handleConfirm() {
    setSubmitting(true)
    const profileIds = [...selectedProfiles]
    let count = 0
    for (const track of tracks) {
      const s = suggestionsByTrack[track.id]
      const sup = supersedeByTrack[track.id]
      const res = supersedeResolutions[track.id]

      let supersedeTrackId: number | undefined
      let supersedeProfileId: number | null | undefined

      if (sup && res !== 'skip') {
        supersedeTrackId = sup.active_track_id
        supersedeProfileId = res === 'discard' ? null : (sup.profile_match?.library_profile_id ?? null)
      }

      try {
        await submitTrack({
          track_id: track.id,
          tag_suggestion_id: s?.id,
          cover_art_url: selectedArtUrl,
          write_folder_art: writeFolderArt,
          profile_ids: profileIds,
          supersede_track_id: supersedeTrackId,
          supersede_profile_id: supersedeProfileId,
        })
        count++
      } catch {
        // continue
      }
    }
    setQueued(count)
    qc.invalidateQueries({ queryKey: ['ingest-staged'] })
    setTimeout(() => onSubmitted(), 1200)
    setSubmitting(false)
  }

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div className="bg-bg-surface border border-border rounded w-[580px] flex flex-col"
        style={{ maxHeight: 'calc(100vh - 4rem)', maxWidth: 'calc(100vw - 2rem)' }}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
          <span className="text-text-primary text-sm font-semibold">Import — {albumKey}</span>
          <button onClick={onClose} className="text-text-muted hover:text-text-primary text-sm leading-none">×</button>
        </div>

        <div className="flex flex-col gap-4 px-4 py-4 overflow-y-auto">
          {/* Track summary — read-only working copy preview */}
          <div>
            <p className="text-text-muted text-xs uppercase tracking-wider mb-2">Track Summary</p>
            {loadingTags ? (
              <p className="text-text-muted text-xs">Loading…</p>
            ) : (
              <div className="border border-border rounded overflow-hidden">
                {trackStatuses.map(({ track, status }) => {
                  const wt = trackWorkingTags[track.id] ?? {}
                  const title = wt.title || getTagValue(track, 'title') || track.relative_path.split('/').pop() || ''
                  const num = wt.tracknumber || getTagValue(track, 'tracknumber') || '—'
                  const missingFields = [...REQUIRED_FIELDS].filter(k => !(wt[k] ?? '').trim())
                  return (
                    <div key={track.id} className="grid grid-cols-[24px_1fr_auto] gap-x-3 items-center px-3 py-1 border-b border-border-subtle last:border-0">
                      <span className="text-text-muted font-mono text-[10px] text-right">{num}</span>
                      <div>
                        <span className="text-text-primary text-xs">{title}</span>
                        {missingFields.length > 0 && (
                          <span className="ml-2 text-[10px] text-amber-400 font-mono">
                            missing: {missingFields.join(', ')}
                          </span>
                        )}
                      </div>
                      <span className={`text-[9px] font-mono uppercase tracking-wide border rounded px-1.5 py-0.5 ${
                        status === 'ready' ? 'text-green-400 border-green-400/30' :
                        status === 'review' ? 'text-amber-400 border-amber-400/30' :
                        'text-text-muted border-border'
                      }`}>
                        {status === 'no-match' ? 'no match' : status}
                      </span>
                    </div>
                  )
                })}
              </div>
            )}
            {unreadyTracks.length > 0 && (
              <p className="mt-1.5 text-xs text-amber-400">
                {unreadyTracks.length} track{unreadyTracks.length !== 1 ? 's' : ''} need review — close and fix before importing
              </p>
            )}
          </div>

          {/* Art */}
          <div>
            <p className="text-text-muted text-xs uppercase tracking-wider mb-2">Cover Art</p>
            <div className="flex items-start gap-3">
              {(uploadedArtUrl || suggestedArtUrl) && artMode === 'use' && (
                <img src={uploadedArtUrl || suggestedArtUrl} alt="cover"
                  className="w-16 h-16 object-cover rounded border border-border flex-shrink-0"
                  onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
                />
              )}
              <div className="flex flex-col gap-2 flex-1 min-w-0">
                <ImageUpload
                  value={uploadedArtUrl}
                  onChange={url => { setUploadedArtUrl(url); setArtMode('use') }}
                />
                <div className="flex items-center gap-3 flex-wrap">
                  {albumHasEmbeddedArt && (
                    <label className="flex items-center gap-1.5 cursor-pointer text-xs">
                      <input type="radio" name="art-mode" checked={artMode === 'keep_embedded'}
                        onChange={() => setArtMode('keep_embedded')} className="accent-[color:var(--accent)]" />
                      <span className={artMode === 'keep_embedded' ? 'text-emerald-400' : 'text-text-muted'}>Keep embedded art</span>
                    </label>
                  )}
                  <label className="flex items-center gap-1.5 cursor-pointer text-xs">
                    <input type="radio" name="art-mode" checked={artMode === 'skip'}
                      onChange={() => setArtMode('skip')} className="accent-[color:var(--accent)]" />
                    <span className={artMode === 'skip' ? 'text-text-primary' : 'text-text-muted'}>Skip art</span>
                  </label>
                  {(artMode === 'keep_embedded' || artMode === 'skip') && (uploadedArtUrl || suggestedArtUrl) && (
                    <button type="button" onClick={() => setArtMode('use')} className="text-xs text-accent hover:underline">
                      Use suggested art
                    </button>
                  )}
                </div>
              </div>
            </div>
            {writeFolderArt && (
              <p className="text-xs text-text-muted mt-1">
                Folder art will be written as <span className="font-mono">{folderArtFilename}</span>
                {artMode === 'keep_embedded' && <span className="text-emerald-400/80"> (extracted from embedded art)</span>}
              </p>
            )}
          </div>

          {/* Supersedes */}
          {supersedeTrackIds.length > 0 && (
            <div>
              <p className="text-text-muted text-xs uppercase tracking-wider mb-2">Supersedes</p>
              <div className="flex flex-col gap-2">
                {supersedeTrackIds.map(track => {
                  const sup = supersedeByTrack[track.id]!
                  const res = supersedeResolutions[track.id]
                  const hasProfile = !!sup.profile_match
                  const isWarning = !hasProfile && (!res || res === 'supersede')
                  return (
                    <div key={track.id} className={`rounded border px-3 py-2 text-xs flex flex-col gap-1.5 ${isWarning ? 'border-amber-400/30 bg-amber-400/5' : 'border-sky-400/20 bg-sky-400/5'}`}>
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-text-primary font-medium truncate">
                          {track.title ?? track.relative_path.split('/').pop()}
                        </span>
                        <span className="text-[10px] text-text-muted shrink-0 font-mono">
                          via {sup.identity_method.replace('_', ' ')}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 text-[11px] font-mono text-text-muted">
                        <span>{sup.active_track_format.toUpperCase()}</span>
                        {sup.active_track_bit_depth != null ? (
                          <><span>{sup.active_track_sample_rate ? `${(sup.active_track_sample_rate / 1000).toFixed(sup.active_track_sample_rate % 1000 === 0 ? 0 : 1)}kHz` : ''}</span><span>{sup.active_track_bit_depth}-bit</span></>
                        ) : (
                          <>{sup.active_track_sample_rate && <span>{(sup.active_track_sample_rate / 1000).toFixed(sup.active_track_sample_rate % 1000 === 0 ? 0 : 1)}kHz</span>}{sup.active_track_bitrate && <span>{sup.active_track_bitrate}k</span>}</>
                        )}
                        <span className="text-text-muted/40 mx-0.5">→</span>
                        {sup.profile_match ? (
                          <span className="text-sky-400">{sup.profile_match.derived_dir_name}<span className="text-text-muted ml-1">({sup.profile_match.profile_name})</span></span>
                        ) : (
                          <span className="text-amber-400">No matching profile</span>
                        )}
                      </div>
                      <div className="flex items-center gap-2 pt-0.5">
                        {hasProfile ? (
                          <div className="flex items-center gap-2">
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input type="radio" name={`sup-${track.id}`} checked={res === 'supersede' || !res}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'supersede' }))}
                                className="accent-[color:var(--accent)]" />
                              <span className="text-text-primary text-xs">Replace → {sup.profile_match!.derived_dir_name}</span>
                            </label>
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input type="radio" name={`sup-${track.id}`} checked={res === 'skip'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'skip' }))}
                                className="accent-[color:var(--accent)]" />
                              <span className="text-text-muted text-xs">Keep existing</span>
                            </label>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input type="radio" name={`sup-${track.id}`} checked={res === 'skip'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'skip' }))}
                                className="accent-[color:var(--accent)]" />
                              <span className="text-text-muted text-xs">Keep existing</span>
                            </label>
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input type="radio" name={`sup-${track.id}`} checked={res === 'discard'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'discard' }))}
                                className="accent-[color:var(--accent)]" />
                              <span className="text-amber-400 text-xs">Replace and discard old file</span>
                            </label>
                            {isWarning && <span className="text-amber-400 text-[10px] ml-auto">Resolve required</span>}
                          </div>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {/* Profiles */}
          {profiles.length > 0 && (
            <div>
              <p className="text-text-muted text-xs uppercase tracking-wider mb-1">Profiles</p>
              <div className="flex flex-col gap-1">
                {profiles.map((p: LibraryProfile) => (
                  <label key={p.id} className="flex items-center gap-2 text-xs cursor-pointer">
                    <input type="checkbox" checked={selectedProfiles.has(p.id)}
                      onChange={() => toggleProfile(p.id)} className="accent-[color:var(--accent)]" />
                    <span className="text-text-primary font-mono">{p.derived_dir_name || `Profile #${p.id}`}</span>
                    {p.auto_include_above_hz != null && (
                      <span className="text-text-muted">≥{(p.auto_include_above_hz / 1000).toFixed(0)}kHz</span>
                    )}
                  </label>
                ))}
              </div>
            </div>
          )}

          {queued > 0 && (
            <p className="text-accent text-xs">Queued {queued} track{queued !== 1 ? 's' : ''}</p>
          )}
        </div>

        <div className="flex justify-end gap-2 px-4 py-3 border-t border-border flex-shrink-0">
          {!canImport && (
            <span className="text-xs text-amber-400 flex items-center mr-auto">
              Resolve {unresolvedWarnings.length} supersede warning{unresolvedWarnings.length !== 1 ? 's' : ''} to continue
            </span>
          )}
          <button onClick={onClose}
            className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary">
            Cancel
          </button>
          <button onClick={handleConfirm} disabled={submitting || !canImport}
            className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50">
            {submitting ? 'Importing…' : `Import ${tracks.length} track${tracks.length !== 1 ? 's' : ''}`}
          </button>
        </div>
      </div>
    </div>
  )
}
