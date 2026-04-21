import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { TagDiffTable } from '../components/TagDiffTable'
import { IngestSearchDialog } from '../components/IngestSearchDialog'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import { getStagedTracks, submitTrack } from '../api/ingest'
import { listLibraryProfiles } from '../api/libraryProfiles'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'
import type { LibraryProfile } from '../types/libraryProfile'

export default function IngestPage() {
  const qc = useQueryClient()
  const [threshold, setThreshold] = useState(80)
  const [searchTrack, setSearchTrack] = useState<Track | null>(null)
  const [submitAlbum, setSubmitAlbum] = useState<string | null>(null)

  const { data: stagedTracks = [], isLoading: tracksLoading } = useQuery({
    queryKey: ['ingest-staged'],
    queryFn: getStagedTracks,
  })

  const { data: suggestions = [] } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
  })

  const batchAccept = useMutation({
    mutationFn: () => tagSuggestionsApi.batchAccept(threshold / 100),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
    },
  })

  const acceptMutation = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.accept(id),
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

  // Build a map of track_id → best pending suggestion
  const suggestionsByTrack: Record<number, TagSuggestion> = {}
  for (const s of suggestions) {
    const existing = suggestionsByTrack[s.track_id]
    if (!existing || s.confidence > existing.confidence) {
      suggestionsByTrack[s.track_id] = s
    }
  }

  // Group tracks by album
  const groups: Record<string, Track[]> = {}
  for (const track of stagedTracks) {
    const key = track.album ?? 'Unknown Album'
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
                onAccept={id => acceptMutation.mutate(id)}
                onReject={id => rejectMutation.mutate(id)}
                onSearch={t => setSearchTrack(t)}
                onSubmitAlbum={key => setSubmitAlbum(key)}
                acceptPending={acceptMutation.isPending ? acceptMutation.variables ?? null : null}
                rejectPending={rejectMutation.isPending ? rejectMutation.variables ?? null : null}
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
          onClose={() => setSubmitAlbum(null)}
          onSubmitted={() => {
            setSubmitAlbum(null)
            qc.invalidateQueries({ queryKey: ['ingest-staged'] })
          }}
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
// AlbumGroup
// ---------------------------------------------------------------------------

function AlbumGroup({
  albumKey,
  tracks,
  suggestionsByTrack,
  onAccept,
  onReject,
  onSearch,
  onSubmitAlbum,
  acceptPending,
  rejectPending,
}: {
  albumKey: string
  tracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  onAccept: (id: number) => void
  onReject: (id: number) => void
  onSearch: (t: Track) => void
  onSubmitAlbum: (key: string) => void
  acceptPending: number | null
  rejectPending: number | null
}) {
  const firstTrack = tracks[0]
  const firstSuggestion = suggestionsByTrack[firstTrack.id]
  const coverArtUrl = firstSuggestion?.cover_art_url
  const formatExt = firstTrack.relative_path.split('.').pop()?.toUpperCase() ?? '?'

  return (
    <div className="border border-border rounded bg-bg-panel">
      {/* Album header */}
      <div className="flex items-center gap-3 px-3 py-2 border-b border-border">
        {coverArtUrl && (
          <img
            src={coverArtUrl}
            alt=""
            className="w-8 h-8 object-cover rounded border border-border flex-shrink-0"
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
        </div>
        <button
          onClick={() => onSubmitAlbum(albumKey)}
          className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 shrink-0"
        >
          Submit Album
        </button>
      </div>

      {/* Track rows */}
      <div className="flex flex-col divide-y divide-border">
        {tracks.map(track => {
          const suggestion = suggestionsByTrack[track.id]
          const pct = suggestion ? Math.round(suggestion.confidence * 100) : null
          return (
            <div key={track.id} className="px-3 py-2 flex flex-col gap-2">
              {/* Track meta row */}
              <div className="flex items-center gap-2">
                <span className="text-text-muted font-mono text-xs w-6 shrink-0">
                  {track.tracknumber ?? '—'}
                </span>
                <span className="text-text-primary text-xs flex-1 truncate">
                  {track.title ?? track.relative_path.split('/').pop()}
                </span>
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
                  {suggestion && (
                    <>
                      <button
                        onClick={() => onAccept(suggestion.id)}
                        disabled={acceptPending === suggestion.id}
                        className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 hover:opacity-90 disabled:opacity-50"
                      >
                        Accept
                      </button>
                      <button
                        onClick={() => onReject(suggestion.id)}
                        disabled={rejectPending === suggestion.id}
                        className="text-xs border border-border rounded px-2 py-0.5 hover:bg-bg-surface disabled:opacity-50"
                      >
                        Reject
                      </button>
                    </>
                  )}
                  <button
                    onClick={() => onSearch(track)}
                    className="text-xs border border-border rounded px-2 py-0.5 hover:bg-bg-surface"
                  >
                    Search
                  </button>
                </div>
              </div>

              {/* Tag diff */}
              <TagDiffTable
                trackId={track.id}
                suggestedTags={suggestion?.suggested_tags ?? {}}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Submit pre-flight dialog
// ---------------------------------------------------------------------------

function SubmitDialog({
  albumKey,
  tracks,
  suggestionsByTrack,
  onClose,
  onSubmitted,
}: {
  albumKey: string
  tracks: Track[]
  suggestionsByTrack: Record<number, TagSuggestion>
  onClose: () => void
  onSubmitted: () => void
}) {
  const qc = useQueryClient()
  const libraryId = tracks[0].library_id

  const { data: profiles = [] } = useQuery({
    queryKey: ['library-profiles', libraryId],
    queryFn: () => listLibraryProfiles(libraryId),
  })

  // Pre-select profiles per spec: include_on_submit AND (auto_include_above_hz is null OR track.sample_rate >= auto_include_above_hz)
  const firstTrack = tracks[0]
  const defaultSelected = new Set(
    profiles
      .filter((p: LibraryProfile) =>
        p.include_on_submit &&
        (p.auto_include_above_hz == null ||
          (firstTrack.sample_rate != null && firstTrack.sample_rate >= p.auto_include_above_hz))
      )
      .map((p: LibraryProfile) => p.id),
  )

  const [selectedProfiles, setSelectedProfiles] = useState<Set<number>>(defaultSelected)
  const [queued, setQueued] = useState(0)
  const [submitting, setSubmitting] = useState(false)

  function toggleProfile(id: number) {
    setSelectedProfiles(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  // Gather representative tags from best accepted/pending suggestion for first track
  const suggestion = suggestionsByTrack[firstTrack.id]
  const coverArtUrl = suggestion?.cover_art_url

  async function handleConfirm() {
    setSubmitting(true)
    const profileIds = [...selectedProfiles]
    let count = 0
    for (const track of tracks) {
      const s = suggestionsByTrack[track.id]
      try {
        await submitTrack({
          track_id: track.id,
          tag_suggestion_id: s?.id,
          cover_art_url: s?.cover_art_url,
          write_folder_art: false,
          profile_ids: profileIds,
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
          <span className="text-text-primary text-sm font-semibold">Submit — {albumKey}</span>
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
          {coverArtUrl && (
            <div>
              <p className="text-text-muted text-xs uppercase tracking-wider mb-1">Cover Art</p>
              <img
                src={coverArtUrl}
                alt="cover"
                className="w-20 h-20 object-cover rounded border border-border"
                onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
              />
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
          <button
            onClick={onClose}
            className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary"
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            disabled={submitting}
            className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
          >
            {submitting ? 'Submitting…' : `Submit ${tracks.length} track${tracks.length !== 1 ? 's' : ''}`}
          </button>
        </div>
      </div>
    </div>
  )
}

