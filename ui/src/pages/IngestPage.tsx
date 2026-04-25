import { useState, useEffect, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { Checkbox } from '../components/Checkbox'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { ImageUpload } from '../components/ImageUpload'
import { TrackEditPanel } from '../components/TrackEditPanel'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import {
  getStagedTracks,
  submitTrack,
  checkSupersede,
  type SupersedeMatchInfo,
} from '../api/ingest'
import { enqueueLookup } from '../api/tracks'
import { listLibraryProfiles } from '../api/libraryProfiles'
import { listSettings } from '../api/settings'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'
import type { LibraryProfile } from '../types/libraryProfile'

// ── Album-scope tag fields (shared across all tracks in an album) ─────────────
interface TagField { key: string; label: string; fullWidth?: boolean }
const ALBUM_EDIT_FIELDS: TagField[] = [
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
  { key: 'musicbrainz_albumartistid',  label: 'MB Release Artist ID', fullWidth: true },
  { key: 'musicbrainz_releasegroupid', label: 'MB Release Group ID',  fullWidth: true },
  { key: 'musicbrainz_releaseid',      label: 'MB Release ID',        fullWidth: true },
]

function getIngestFolder(relativePath: string): string {
  const stripped = relativePath.replace(/^ingest\//, '')
  const lastSlash = stripped.lastIndexOf('/')
  return lastSlash === -1 ? '(root)' : stripped.slice(0, lastSlash)
}

export default function IngestPage() {
  const qc = useQueryClient()
  const [threshold, setThreshold] = useState(80)
  const [groupMode, setGroupMode] = useState<'album' | 'folder'>('album')
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)
  const [submitAlbum, setSubmitAlbum] = useState<string | null>(null)
  const [editingTrackId, setEditingTrackId] = useState<number | null>(null)
  const [albumArtUrls, setAlbumArtUrls] = useState<Record<string, string>>({})

  const { data: stagedTracks = [], isLoading: tracksLoading } = useQuery({
    queryKey: ['ingest-staged'],
    queryFn: getStagedTracks,
  })

  const { data: suggestions = [] } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
  })

  // Supersede check — runs whenever staged tracks change
  const { data: supersedeResults = [] } = useQuery({
    queryKey: ['ingest-supersede', stagedTracks.map(t => t.id)],
    queryFn: () =>
      stagedTracks.length > 0
        ? checkSupersede(stagedTracks.map(t => t.id))
        : Promise.resolve([]),
    enabled: stagedTracks.length > 0,
  })

  // Build map: track_id → SupersedeMatchInfo
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

  const acceptMutation = useMutation({
    mutationFn: ({ id, fields, applyArt }: { id: number; fields?: string[]; applyArt?: boolean }) =>
      tagSuggestionsApi.accept(id, fields, applyArt),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
    },
  })

  const rejectMutation = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.reject(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
    },
  })

  const lookupMutation = useMutation({
    mutationFn: (trackId: number) => enqueueLookup(trackId),
  })

  // Build a map of track_id → best pending suggestion
  const suggestionsByTrack: Record<number, TagSuggestion> = {}
  for (const s of suggestions) {
    const existing = suggestionsByTrack[s.track_id]
    if (!existing || s.confidence > existing.confidence) {
      suggestionsByTrack[s.track_id] = s
    }
  }

  // Group tracks by album or by ingest folder
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
        {/* Batch accept bar */}
        <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-bg-surface flex-shrink-0">
          <span className="text-xs text-text-muted">
            {stagedTracks.length === 0
              ? 'No staged tracks'
              : `${stagedTracks.length} staged track${stagedTracks.length !== 1 ? 's' : ''}`}
          </span>
          {/* Group mode toggle */}
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
              type="number"
              min={1}
              max={100}
              value={threshold}
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

        {/* Album groups */}
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
                onAccept={(id, fields, applyArt) => acceptMutation.mutate({ id, fields, applyArt })}
                onReject={id => rejectMutation.mutate(id)}
                onSearch={t => setSearchTrack(t)}
                onLookup={id => lookupMutation.mutate(id)}
                onSubmitAlbum={key => setSubmitAlbum(key)}
                acceptPending={acceptMutation.isPending ? (acceptMutation.variables?.id ?? null) : null}
                rejectPending={rejectMutation.isPending ? rejectMutation.variables ?? null : null}
                lookupPending={lookupMutation.isPending ? lookupMutation.variables ?? null : null}
                editingTrackId={editingTrackId}
                onEdit={id => setEditingTrackId(id)}
                onEditClose={() => setEditingTrackId(null)}
                presetArtUrl={albumArtUrls[albumKey] ?? ''}
                onArtChange={url => setAlbumArtUrls(prev => ({ ...prev, [albumKey]: url }))}
              />
            ))
          )}
        </div>
      </div>

      {/* Submit pre-flight dialog */}
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

      {/* Manual search dialog */}
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

// ---------------------------------------------------------------------------
// IngestDiffPanel — interactive field-selection diff (mirrors SuggestionReviewPane)
// ---------------------------------------------------------------------------

const INGEST_TOP_LEVEL = new Set([
  'title', 'artist', 'albumartist', 'album', 'tracknumber', 'discnumber',
  'totaldiscs', 'totaltracks', 'date', 'genre', 'label', 'catalognumber',
])

function getTagValue(track: Track, key: string): string {
  if (INGEST_TOP_LEVEL.has(key)) {
    return (track as unknown as Record<string, string | undefined>)[key] ?? ''
  }
  const v = (track.tags as Record<string, unknown>)[key]
  if (typeof v === 'string') return v
  if (v != null) return String(v)
  return ''
}

function IngestDiffPanel({
  track,
  suggestion,
  applying,
  rejecting,
  onApply,
  onReject,
  overrideTags,
  overrideArtUrl,
}: {
  track: Track
  suggestion: TagSuggestion
  applying: boolean
  rejecting: boolean
  onApply: (fields?: string[], applyArt?: boolean) => void
  onReject: () => void
  overrideTags?: Record<string, string>
  overrideArtUrl?: string | null
}) {
  const effectiveTags = overrideTags ?? (suggestion.suggested_tags as Record<string, string>)
  const effectiveArtUrl = overrideArtUrl !== undefined ? overrideArtUrl : (suggestion.cover_art_url ?? null)

  const diffItems = useMemo(() => {
    const suggested = (effectiveTags ?? {}) as Record<string, unknown>
    return Object.entries(suggested).map(([key, raw]) => {
      const suggestedVal = typeof raw === 'string' ? raw : String(raw ?? '')
      const currentVal = getTagValue(track, key)
      return { key, currentVal, suggestedVal, changed: currentVal !== suggestedVal }
    })
  }, [effectiveTags, track])

  const [checkedFields, setCheckedFields] = useState<Set<string>>(
    () => new Set(diffItems.filter(d => d.changed).map(d => d.key))
  )
  const [applyArt, setApplyArt] = useState(() => !!effectiveArtUrl)

  const allChecked = diffItems.length > 0 && checkedFields.size === diffItems.length
  const noneChecked = checkedFields.size === 0 && !applyArt

  function toggleField(key: string) {
    setCheckedFields(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key); else next.add(key)
      return next
    })
  }

  function handleApply() {
    const fields = [...checkedFields]
    onApply(fields.length < diffItems.length ? fields : undefined, applyArt)
  }

  return (
    <div className="border border-border rounded bg-bg-base text-xs">
      {/* Panel header */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-bg-panel">
        <button
          onClick={() => setCheckedFields(allChecked ? new Set() : new Set(diffItems.map(d => d.key)))}
          className="text-[11px] text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5"
        >
          {allChecked ? 'None' : 'All'}
        </button>
        <span className="flex-1" />
        <button
          onClick={onReject}
          disabled={rejecting || applying}
          className="text-[11px] text-text-muted hover:text-destructive border border-border rounded px-2 py-0.5 disabled:opacity-40"
        >
          Reject
        </button>
        <button
          onClick={handleApply}
          disabled={applying || noneChecked}
          className="text-[11px] bg-accent text-bg-base rounded px-3 py-0.5 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {applying ? 'Applying…' : `Apply${checkedFields.size < diffItems.length ? ` (${checkedFields.size})` : ''}`}
        </button>
      </div>
      {/* Art row */}
      {effectiveArtUrl && (
        <div
          className="grid grid-cols-[6rem_1fr_1fr_1.5rem] gap-x-2 px-3 py-1 border-b border-border-subtle items-center cursor-pointer select-none hover:bg-bg-row-hover"
          onClick={() => setApplyArt(v => !v)}
        >
          <span className="text-[11px] text-text-muted">Cover Art</span>
          <span className="text-text-secondary font-mono">{track.has_embedded_art ? 'embedded' : '—'}</span>
          <img src={effectiveArtUrl} alt="" className="w-7 h-7 object-cover rounded" />
          <span className="flex items-center justify-center" onClick={e => e.stopPropagation()}>
            <Checkbox checked={applyArt} onChange={() => setApplyArt(v => !v)} />
          </span>
        </div>
      )}
      {/* Field rows */}
      {diffItems.map(({ key, currentVal, suggestedVal, changed }) => (
        <div
          key={key}
          className={`grid grid-cols-[6rem_1fr_1fr_1.5rem] gap-x-2 px-3 py-0.5 border-b border-border-subtle items-center cursor-pointer select-none ${changed ? 'hover:bg-bg-row-hover' : 'opacity-40'}`}
          onClick={() => changed && toggleField(key)}
        >
          <span className="text-[11px] text-text-muted truncate font-mono" title={key}>{key}</span>
          <span className={`text-[11px] truncate font-mono ${changed ? 'text-text-muted line-through' : 'text-text-secondary'}`}>
            {currentVal || <em className="not-italic text-text-muted/40">—</em>}
          </span>
          <span className={`text-[11px] truncate font-mono ${changed ? 'text-text-primary' : 'text-text-secondary'}`}>
            {suggestedVal || <em className="not-italic text-text-muted/40">—</em>}
          </span>
          <span className="flex items-center justify-center" onClick={e => e.stopPropagation()}>
            {changed && <Checkbox checked={checkedFields.has(key)} onChange={() => toggleField(key)} />}
          </span>
        </div>
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// AlbumGroup
// ---------------------------------------------------------------------------

function AlbumGroup({
  albumKey,
  tracks,
  suggestionsByTrack,
  supersedeByTrack,
  onAccept,
  onReject,
  onSearch,
  onLookup,
  onSubmitAlbum,
  acceptPending,
  rejectPending,
  lookupPending,
  editingTrackId,
  onEdit,
  onEditClose,
  presetArtUrl,
  onArtChange,
}: {
  albumKey: string
  tracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  supersedeByTrack: Record<number, SupersedeMatchInfo>
  onAccept: (id: number, fields?: string[], applyArt?: boolean) => void
  onReject: (id: number) => void
  onSearch: (t: Track) => void
  onLookup: (id: number) => void
  onSubmitAlbum: (key: string) => void
  acceptPending: number | null
  rejectPending: number | null
  lookupPending: number | null
  editingTrackId: number | null
  onEdit: (id: number) => void
  onEditClose: () => void
  presetArtUrl: string
  onArtChange: (url: string) => void
}) {
  const qc = useQueryClient()
  const firstTrack = tracks[0]
  const firstSuggestion = suggestionsByTrack[firstTrack.id]
  const coverArtUrl = firstSuggestion?.cover_art_url
  const displayArtUrl = presetArtUrl || coverArtUrl
  const hasEmbeddedArt = tracks.some(t => t.has_embedded_art)
  const formatExt = firstTrack.relative_path.split('.').pop()?.toUpperCase() ?? '?'
  const [altIdxByTrack, setAltIdxByTrack] = useState<Record<number, number | null>>({})
  const [editingAlbum, setEditingAlbum] = useState(false)
  const [acceptedTrackIds, setAcceptedTrackIds] = useState<Set<number>>(new Set())
  const [selectedAltIdx, setSelectedAltIdx] = useState<number | null>(null)

  // Collect album-level alternatives from any suggestion that has them
  const albumSugWithAlts = Object.values(suggestionsByTrack).find(s => (s.alternatives?.length ?? 0) > 0)
  const albumAlternatives = albumSugWithAlts?.alternatives ?? []
  const primaryAlbumLabel =
    albumSugWithAlts?.suggested_tags?.album ?? firstTrack.album ?? 'Current release'

  function handleAcceptTrack(suggestionId: number, trackId: number, fields?: string[], applyArt?: boolean) {
    onAccept(suggestionId, fields, applyArt)
    setAcceptedTrackIds(prev => new Set([...prev, trackId]))
  }

  async function handleAcceptTrackWithAlt(
    suggestion: TagSuggestion,
    trackId: number,
    altIdx: number,
    fields?: string[],
    applyArt?: boolean,
  ) {
    const alt = suggestion.alternatives?.[altIdx]
    if (!alt) { handleAcceptTrack(suggestion.id, trackId, fields, applyArt); return }
    try {
      const newSug = await tagSuggestionsApi.create({
        track_id: suggestion.track_id,
        source: suggestion.source,
        suggested_tags: alt.suggested_tags,
        confidence: suggestion.confidence,
        cover_art_url: alt.cover_art_url,
        musicbrainz_release_id: alt.mb_release_id,
        musicbrainz_recording_id: suggestion.mb_recording_id,
      })
      await tagSuggestionsApi.accept(newSug.id, fields, applyArt ?? false)
      await tagSuggestionsApi.reject(suggestion.id).catch(() => {})
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
      setAcceptedTrackIds(prev => new Set([...prev, trackId]))
    } catch { /* ignore */ }
  }
  const [showArtUpload, setShowArtUpload] = useState(false)
  const [expandedSupersede, setExpandedSupersede] = useState<number | null>(null)

  const supersedeCount = tracks.filter(t => supersedeByTrack[t.id]).length

  return (
    <div className="border border-border rounded bg-bg-panel">
      {/* Album header */}
      <div className="flex items-center gap-3 px-3 py-2 border-b border-border">
        {displayArtUrl && (
          <img
            src={displayArtUrl}
            alt=""
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
              title="Switch to an alternate release"
            >
              <option value="">{primaryAlbumLabel}</option>
              {albumAlternatives.map((alt, i) => {
                const name = alt.suggested_tags.album ?? alt.mb_release_id
                const date = alt.suggested_tags.date ? ` (${alt.suggested_tags.date})` : ''
                const artist = alt.suggested_tags.albumartist ? ` · ${alt.suggested_tags.albumartist}` : ''
                return (
                  <option key={i} value={String(i)}>{name}{date}{artist}</option>
                )
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

      {/* Inline art upload panel */}
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
            <button
              type="button"
              onClick={() => { onArtChange(''); setShowArtUpload(false) }}
              className="text-xs text-text-muted border border-border rounded px-2 py-0.5 hover:text-destructive shrink-0"
            >
              Remove
            </button>
          )}
          <button
            type="button"
            onClick={() => setShowArtUpload(false)}
            className="text-xs text-text-muted border border-border rounded px-2 py-0.5 hover:text-text-primary shrink-0"
          >
            Close
          </button>
        </div>
      )}

      {/* Album bulk-edit panel */}
      {editingAlbum && (
        <AlbumEditPanel
          tracks={tracks}
          onClose={() => setEditingAlbum(false)}
        />
      )}

      {/* Track rows */}
      <div className="flex flex-col divide-y divide-border">
        {tracks.map(track => {
          const suggestion = suggestionsByTrack[track.id]
          const supersede = supersedeByTrack[track.id]
          const pct = suggestion ? Math.round(suggestion.confidence * 100) : null
          const isEditing = editingTrackId === track.id
          const supersedeExpanded = expandedSupersede === track.id
          const isAccepted = acceptedTrackIds.has(track.id)

          if (isAccepted) {
            return (
              <div key={track.id} className="px-3 py-1.5 flex items-center gap-2 opacity-50">
                <span className="text-text-muted font-mono text-xs w-6 shrink-0">{track.tracknumber ?? '—'}</span>
                <span className="text-text-secondary text-xs flex-1 truncate">
                  {track.title ?? track.relative_path.split('/').pop()}
                </span>
                <span className="text-[10px] font-mono text-green-400 border border-green-400/40 rounded px-1.5 py-0.5">✓ Accepted</span>
              </div>
            )
          }

          // Hoist alt override computation so both TrackEditPanel and IngestDiffPanel use the same values
          const trackAltIdx = altIdxByTrack[track.id] ?? null
          const effectiveAltIdx = suggestion && suggestion.confidence < 1.0
            ? (trackAltIdx !== null ? trackAltIdx : selectedAltIdx)
            : null
          const altOverride = effectiveAltIdx !== null && suggestion
            ? suggestion.alternatives?.[effectiveAltIdx]
            : undefined

          return (
            <div key={track.id} className="px-3 py-2 flex flex-col gap-2">
              {/* Track meta row */}
              <div className="flex items-center gap-2">
                <span className="text-text-muted font-mono text-xs w-6 shrink-0">
                  {track.tracknumber ?? '—'}
                </span>
                <span className="flex-1 min-w-0 flex flex-col">
                  <span className="text-text-primary text-xs truncate">
                    {track.title ?? track.relative_path.split('/').pop()}
                  </span>
                  {track.title && (
                    <span className="text-text-muted font-mono text-[10px] truncate">
                      {track.relative_path.split('/').pop()}
                    </span>
                  )}
                </span>
                {/* Supersede badge */}
                {supersede && (
                  <button
                    onClick={() => setExpandedSupersede(supersedeExpanded ? null : track.id)}
                    className={`text-[10px] font-mono uppercase rounded px-1 shrink-0 border ${
                      supersede.profile_match
                        ? 'text-sky-400 border-sky-400/40 hover:border-sky-400'
                        : 'text-amber-400 border-amber-400/40 hover:border-amber-400'
                    }`}
                    title={supersede.profile_match ? 'Replaces existing — click to expand' : 'Replaces existing — no matching profile'}
                  >
                    {supersede.profile_match ? 'Replaces existing' : '⚠ Replaces existing'}
                  </button>
                )}
                {pct != null && (
                  <span className={`text-[10px] font-mono shrink-0 ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>
                    {pct}%
                  </span>
                )}
                {suggestion && (
                  <span className="text-[10px] uppercase tracking-wide text-text-muted font-mono shrink-0">
                    {suggestion.source}
                  </span>
                )}
                {/* Actions */}
                <div className="flex items-center gap-1 shrink-0">
                  <button
                    onClick={() => isEditing ? onEditClose() : onEdit(track.id)}
                    className={`text-xs border rounded px-2 py-0.5 hover:bg-bg-surface ${isEditing ? 'border-accent text-accent' : 'border-border text-text-muted'}`}
                  >
                    Edit
                  </button>
                  {suggestion?.alternatives && suggestion.alternatives.length > 0 && (
                    <select
                      value={altIdxByTrack[track.id] == null ? '' : String(altIdxByTrack[track.id])}
                      onChange={e => {
                        const val = e.target.value === '' ? null : Number(e.target.value)
                        setAltIdxByTrack(prev => ({ ...prev, [track.id]: val }))
                      }}
                      className="text-[11px] font-mono bg-bg-base border border-border rounded px-1.5 py-0.5 text-text-primary focus:outline-none focus:border-accent max-w-[160px]"
                      title="Select alternate release for this track"
                    >
                      <option value="">{suggestion.suggested_tags?.album ?? track.album ?? 'Current'}</option>
                      {suggestion.alternatives.map((alt, i) => {
                        const name = alt.suggested_tags.album ?? alt.mb_release_id
                        const date = alt.suggested_tags.date ? ` (${alt.suggested_tags.date})` : ''
                        return <option key={i} value={String(i)}>{name}{date}</option>
                      })}
                    </select>
                  )}
                  <button
                    onClick={() => onSearch(track)}
                    className="text-xs border border-border text-text-muted rounded px-2 py-0.5 hover:bg-bg-surface"
                  >
                    Search
                  </button>
                  <button
                    onClick={() => onLookup(track.id)}
                    disabled={lookupPending === track.id}
                    className="text-xs border border-border text-text-muted rounded px-2 py-0.5 hover:bg-bg-surface disabled:opacity-50"
                  >
                    {lookupPending === track.id ? 'Queued…' : 'Lookup'}
                  </button>
                </div>
              </div>

              {/* Supersede detail row */}
              {supersedeExpanded && supersede && (
                <SupersedeDetailRow supersede={supersede} />
              )}

              {/* Inline edit panel */}
              {isEditing && suggestion && (
                <TrackEditPanel
                  key={`${suggestion.id}-${effectiveAltIdx ?? 'none'}`}
                  track={track}
                  suggestion={suggestion}
                  overrideTags={altOverride?.suggested_tags}
                  onClose={onEditClose}
                />
              )}
              {isEditing && !suggestion && (
                <TrackEditPanel
                  track={track}
                  suggestion={undefined}
                  onClose={onEditClose}
                />
              )}

              {/* Tag diff with field selection */}
              {!isEditing && suggestion && (
                <IngestDiffPanel
                  key={`${suggestion.id}-${effectiveAltIdx ?? 'none'}`}
                  track={track}
                  suggestion={suggestion}
                  applying={acceptPending === suggestion.id}
                  rejecting={rejectPending === suggestion.id}
                  onApply={(fields, applyArt) =>
                    effectiveAltIdx !== null
                      ? handleAcceptTrackWithAlt(suggestion, track.id, effectiveAltIdx, fields, applyArt)
                      : handleAcceptTrack(suggestion.id, track.id, fields, applyArt)
                  }
                  onReject={() => onReject(suggestion.id)}
                  overrideTags={altOverride?.suggested_tags}
                  overrideArtUrl={altOverride !== undefined ? (altOverride.cover_art_url || null) : undefined}
                />
              )}
              {!isEditing && !suggestion && (
                <p className="text-text-muted text-[11px] italic px-1">
                  No lookup results — click <strong className="font-semibold not-italic text-text-secondary">Lookup</strong> to run fingerprint matching,{' '}
                  <strong className="font-semibold not-italic text-text-secondary">Search</strong> to find manually, or{' '}
                  <strong className="font-semibold not-italic text-text-secondary">Edit</strong> to enter tags directly.
                </p>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Supersede detail row (inline expand in track list)
// ---------------------------------------------------------------------------

function SupersedeDetailRow({ supersede }: { supersede: SupersedeMatchInfo }) {
  const fmtQuality = (fmt: string, sr: number | null, bd: number | null, br: number | null) => {
    const fmtStr = fmt.toUpperCase()
    const khz = sr != null ? `${(sr / 1000).toFixed(sr % 1000 === 0 ? 0 : 1)}kHz` : null
    if (bd != null) {
      const parts = [fmtStr]
      if (khz) parts.push(khz)
      parts.push(`${bd}-bit`)
      return parts.join(' · ')
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
            {fmtQuality(
              supersede.active_track_format,
              supersede.active_track_sample_rate,
              supersede.active_track_bit_depth,
              supersede.active_track_bitrate,
            )}
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

// ---------------------------------------------------------------------------
// Album bulk-edit panel
// ---------------------------------------------------------------------------

function AlbumEditPanel({
  tracks,
  onClose,
}: {
  tracks: Track[]
  onClose: () => void
}) {
  const qc = useQueryClient()
  // Derive consensus current value per field across all tracks
  const currentValues = useMemo<Record<string, string>>(() => {
    const result: Record<string, string> = {}
    for (const { key } of ALBUM_EDIT_FIELDS) {
      const vals = tracks.map(t => getAlbumTagValue(t, key)).filter(v => v !== '')
      const unique = [...new Set(vals)]
      result[key] = unique.length === 1 ? unique[0] : unique.length > 1 ? '(mixed)' : ''
    }
    return result
  }, [tracks])

  const [fields, setFields] = useState<Record<string, string>>(
    () => Object.fromEntries(ALBUM_EDIT_FIELDS.map(f => [f.key, '']))
  )
  const [saving, setSaving] = useState(false)
  const [savedCount, setSavedCount] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function handleApply() {
    // New value takes priority; fall back to current consensus value (skip "(mixed)" and blank)
    const tags: Record<string, string> = {}
    for (const { key } of ALBUM_EDIT_FIELDS) {
      const newVal = fields[key].trim()
      const cur = currentValues[key]
      const val = newVal !== '' ? newVal : (cur !== '' && cur !== '(mixed)' ? cur : '')
      if (val !== '') tags[key] = val
    }
    if (Object.keys(tags).length === 0) return

    setSaving(true)
    setError(null)
    setSavedCount(null)
    let count = 0
    const errors: string[] = []
    for (const track of tracks) {
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

  // Count fields that will push a value (new override OR current consensus)
  const pushCount = ALBUM_EDIT_FIELDS.filter(({ key }) => {
    const newVal = fields[key].trim()
    const cur = currentValues[key]
    return newVal !== '' || (cur !== '' && cur !== '(mixed)')
  }).length

  return (
    <div className="border-b border-border bg-bg-base text-xs">
      {/* Panel header */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-bg-panel">
        <span className="text-[11px] text-text-muted font-mono">
          Album Tags — applies to all {tracks.length} tracks
        </span>
        <span className="flex-1" />
        {savedCount != null && (
          <span className="text-[11px] text-green-400">Applied to {savedCount} tracks</span>
        )}
        {error && <span className="text-[11px] text-destructive">{error}</span>}
        <button
          type="button"
          onClick={onClose}
          className="text-[11px] text-text-muted hover:text-text-primary border border-border rounded px-2 py-0.5"
        >
          Close
        </button>
        <button
          type="button"
          onClick={handleApply}
          disabled={saving || pushCount === 0}
          className="text-[11px] bg-accent text-bg-base rounded px-3 py-0.5 font-medium hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {saving ? 'Applying…' : `Apply to All (${pushCount})`}
        </button>
      </div>
      {/* Diff rows: field | current | new (editable) */}
      {ALBUM_EDIT_FIELDS.map(({ key, label }) => {
        const current = currentValues[key]
        const newVal = fields[key]
        const hasNew = newVal.trim() !== ''
        return (
          <div
            key={key}
            className="grid grid-cols-[7rem_1fr_1fr] gap-x-2 px-3 py-0.5 border-b border-border-subtle items-center"
          >
            <span className="text-[11px] text-text-muted font-mono truncate" title={label}>{label}</span>
            <span className={`text-[11px] font-mono truncate ${hasNew ? 'text-text-muted line-through' : 'text-text-secondary'}`}>
              {current || <em className="not-italic text-text-muted/40">—</em>}
            </span>
            <input
              type="text"
              value={newVal}
              placeholder="(unchanged)"
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

function getAlbumTagValue(track: Track, key: string): string {
  const albumKeys = new Set(ALBUM_EDIT_FIELDS.map(f => f.key))
  if (albumKeys.has(key)) {
    const topLevel = (track as unknown as Record<string, string | undefined>)[key]
    if (topLevel != null) return topLevel
    const v = (track.tags as Record<string, unknown>)[key]
    if (typeof v === 'string') return v
    if (v != null) return String(v)
  }
  return ''
}

// ---------------------------------------------------------------------------
// Import pre-flight dialog
// ---------------------------------------------------------------------------

// Per-track supersede resolution state
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

  // Per-track supersede resolution; default = 'supersede' if profile matched, else must be resolved
  const [supersedeResolutions, setSupersedeResolutions] = useState<Record<number, SupersedeResolution>>({})

  // Initialise resolutions when supersede data is available
  useEffect(() => {
    const initial: Record<number, SupersedeResolution> = {}
    for (const track of tracks) {
      const s = supersedeByTrack[track.id]
      if (s) {
        // Auto-select 'supersede' if there's a matching profile; unresolved (no default) if not
        initial[track.id] = s.profile_match ? 'supersede' : ('supersede' as SupersedeResolution)
      }
    }
    setSupersedeResolutions(initial)
  }, [tracks.map(t => t.id).join(','), Object.keys(supersedeByTrack).join(',')])

  // Tracks that have a supersede candidate
  const supersedeTrackIds = tracks.filter(t => supersedeByTrack[t.id])
  // Tracks that are unresolved (no matching profile and no explicit resolution yet)
  const unresolvedWarnings = supersedeTrackIds.filter(t => {
    const s = supersedeByTrack[t.id]
    if (!s || s.profile_match) return false
    const res = supersedeResolutions[t.id]
    return !res || res === 'supersede' // 'supersede' without a profile = unresolved
  })
  const canImport = unresolvedWarnings.length === 0

  function toggleProfile(id: number) {
    setSelectedProfiles(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const suggestion = suggestionsByTrack[firstTrack.id]
  const suggestedArtUrl = suggestion?.cover_art_url
  const albumHasEmbeddedArt = tracks.some(t => t.has_embedded_art)

  // 'use' = use uploaded/suggested art | 'keep_embedded' = keep embedded | 'skip' = no art
  type ArtMode = 'use' | 'keep_embedded' | 'skip'
  const defaultArtMode: ArtMode =
    !uploadedArtUrl && !suggestedArtUrl && albumHasEmbeddedArt ? 'keep_embedded' : 'use'
  const [artMode, setArtMode] = useState<ArtMode>(defaultArtMode)

  const selectedArtUrl: string | undefined =
    artMode === 'skip' || artMode === 'keep_embedded'
      ? undefined
      : (uploadedArtUrl || suggestedArtUrl || undefined)

  // write_folder_art: true when there is art to write (explicit URL or embedded extraction)
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
        if (res === 'discard') {
          supersedeProfileId = null // explicit discard
        } else {
          // 'supersede' — use matched profile if available
          supersedeProfileId = sup.profile_match?.library_profile_id ?? null
        }
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
      <div className="bg-bg-surface border border-border rounded w-[560px] flex flex-col"
        style={{ maxHeight: 'calc(100vh - 4rem)', maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
          <span className="text-text-primary text-sm font-semibold">Import — {albumKey}</span>
          <button onClick={onClose} className="text-text-muted hover:text-text-primary text-sm leading-none">×</button>
        </div>

        <div className="flex flex-col gap-4 px-4 py-4 overflow-y-auto">
          {/* Tags summary */}
          {suggestion && (
            <div>
              <p className="text-text-muted text-xs uppercase tracking-wider mb-1">Tags</p>
              <div className="flex flex-col gap-0.5 text-xs">
                {Object.entries(suggestion.suggested_tags).slice(0, 10).map(([k, v]) => (
                  <div key={k} className="flex gap-2">
                    <span className="text-text-muted font-mono w-36 shrink-0 truncate">{k}</span>
                    <span className="text-text-primary truncate">{v}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Art panel */}
          <div>
            <p className="text-text-muted text-xs uppercase tracking-wider mb-2">Cover Art</p>
            <div className="flex items-start gap-3">
              {(uploadedArtUrl || suggestedArtUrl) && artMode === 'use' && (
                <img
                  src={uploadedArtUrl || suggestedArtUrl}
                  alt="cover"
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
                      <input
                        type="radio"
                        name="art-mode"
                        checked={artMode === 'keep_embedded'}
                        onChange={() => setArtMode('keep_embedded')}
                        className="accent-[color:var(--accent)]"
                      />
                      <span className={artMode === 'keep_embedded' ? 'text-emerald-400' : 'text-text-muted'}>
                        Keep embedded art
                      </span>
                    </label>
                  )}
                  <label className="flex items-center gap-1.5 cursor-pointer text-xs">
                    <input
                      type="radio"
                      name="art-mode"
                      checked={artMode === 'skip'}
                      onChange={() => setArtMode('skip')}
                      className="accent-[color:var(--accent)]"
                    />
                    <span className={artMode === 'skip' ? 'text-text-primary' : 'text-text-muted'}>Skip art</span>
                  </label>
                  {(artMode === 'keep_embedded' || artMode === 'skip') && (uploadedArtUrl || suggestedArtUrl) && (
                    <button
                      type="button"
                      onClick={() => setArtMode('use')}
                      className="text-xs text-accent hover:underline"
                    >
                      Use suggested art
                    </button>
                  )}
                </div>
              </div>
            </div>
            {writeFolderArt && (
              <p className="text-xs text-text-muted mt-1">
                Folder art will be written as{' '}
                <span className="font-mono">{folderArtFilename}</span>
                {artMode === 'keep_embedded' && (
                  <span className="text-emerald-400/80"> (extracted from embedded art)</span>
                )}
              </p>
            )}
          </div>

          {/* Supersedes section */}
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
                    <div
                      key={track.id}
                      className={`rounded border px-3 py-2 text-xs flex flex-col gap-1.5 ${
                        isWarning ? 'border-amber-400/30 bg-amber-400/5' : 'border-sky-400/20 bg-sky-400/5'
                      }`}
                    >
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
                          <>
                            {sup.active_track_sample_rate && <span>{(sup.active_track_sample_rate / 1000).toFixed(sup.active_track_sample_rate % 1000 === 0 ? 0 : 1)}kHz</span>}
                            <span>{sup.active_track_bit_depth}-bit</span>
                          </>
                        ) : sup.active_track_sample_rate && sup.active_track_bitrate ? (
                          <span>{(sup.active_track_sample_rate / 1000).toFixed(sup.active_track_sample_rate % 1000 === 0 ? 0 : 1)}kHz / {sup.active_track_bitrate}k</span>
                        ) : (
                          <>
                            {sup.active_track_sample_rate && <span>{(sup.active_track_sample_rate / 1000).toFixed(sup.active_track_sample_rate % 1000 === 0 ? 0 : 1)}kHz</span>}
                            {sup.active_track_bitrate && <span>{sup.active_track_bitrate}k</span>}
                          </>
                        )}
                        <span className="text-text-muted/40 mx-0.5">→</span>
                        {sup.profile_match ? (
                          <span className="text-sky-400">
                            {sup.profile_match.derived_dir_name}
                            <span className="text-text-muted ml-1">({sup.profile_match.profile_name})</span>
                          </span>
                        ) : (
                          <span className="text-amber-400">No matching profile</span>
                        )}
                      </div>
                      {/* Resolution controls */}
                      <div className="flex items-center gap-2 pt-0.5">
                        {hasProfile ? (
                          <div className="flex items-center gap-2">
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name={`sup-${track.id}`}
                                checked={res === 'supersede' || !res}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'supersede' }))}
                                className="accent-[color:var(--accent)]"
                              />
                              <span className="text-text-primary">Replace → {sup.profile_match!.derived_dir_name}</span>
                            </label>
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name={`sup-${track.id}`}
                                checked={res === 'skip'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'skip' }))}
                                className="accent-[color:var(--accent)]"
                              />
                              <span className="text-text-muted">Keep existing</span>
                            </label>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name={`sup-${track.id}`}
                                checked={res === 'skip'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'skip' }))}
                                className="accent-[color:var(--accent)]"
                              />
                              <span className="text-text-muted">Keep existing</span>
                            </label>
                            <label className="flex items-center gap-1.5 cursor-pointer">
                              <input
                                type="radio"
                                name={`sup-${track.id}`}
                                checked={res === 'discard'}
                                onChange={() => setSupersedeResolutions(prev => ({ ...prev, [track.id]: 'discard' }))}
                                className="accent-[color:var(--accent)]"
                              />
                              <span className="text-amber-400">Replace and discard old file</span>
                            </label>
                            {isWarning && (
                              <span className="text-amber-400 text-[10px] ml-auto">Resolve required</span>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {/* Profile checklist */}
          {profiles.length > 0 && (
            <div>
              <p className="text-text-muted text-xs uppercase tracking-wider mb-1">Profiles</p>
              <div className="flex flex-col gap-1">
                {profiles.map((p: LibraryProfile) => (
                  <label key={p.id} className="flex items-center gap-2 text-xs cursor-pointer">
                    <input
                      type="checkbox"
                      checked={selectedProfiles.has(p.id)}
                      onChange={() => toggleProfile(p.id)}
                      className="accent-[color:var(--accent)]"
                    />
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
          <button
            onClick={onClose}
            className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary"
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            disabled={submitting || !canImport}
            className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
          >
            {submitting ? 'Importing…' : `Import ${tracks.length} track${tracks.length !== 1 ? 's' : ''}`}
          </button>
        </div>
      </div>
    </div>
  )
}
