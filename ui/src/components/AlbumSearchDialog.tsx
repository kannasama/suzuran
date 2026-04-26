import { useState } from 'react'
import { searchMbRelease, getMbRelease } from '../api/search'
import type { MbReleaseSummary, MbReleaseFull } from '../api/search'
import { getPendingTags, setPendingTags } from '../api/tracks'
import type { Track } from '../types/track'

type Tab = 'search' | 'release-id'

function getTrackNumber(track: Track): string | null {
  const fromTags = (track.tags as Record<string, unknown>)?.tracknumber
  if (typeof fromTags === 'string' && fromTags) return fromTags
  if (track.tracknumber != null) return String(track.tracknumber)
  return null
}

interface Props {
  tracks: Track[]
  onClose: () => void
  onApplied: () => void
}

export function AlbumSearchDialog({ tracks, onClose, onApplied }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>('search')

  // Search tab state
  const firstTrack = tracks[0]
  const [srArtist, setSrArtist] = useState(firstTrack?.artist ?? '')
  const [srAlbum, setSrAlbum] = useState(firstTrack?.album ?? '')
  const [srResults, setSrResults] = useState<MbReleaseSummary[]>([])
  const [srLoading, setSrLoading] = useState(false)
  const [srError, setSrError] = useState<string | null>(null)
  const [srSearched, setSrSearched] = useState(false)

  // Release ID tab state
  const [ridValue, setRidValue] = useState('')
  const [ridRelease, setRidRelease] = useState<MbReleaseFull | null>(null)
  const [ridLoading, setRidLoading] = useState(false)
  const [ridError, setRidError] = useState<string | null>(null)

  // Applying state (shared)
  const [applying, setApplying] = useState<string | null>(null) // release id being applied
  const [applyError, setApplyError] = useState<string | null>(null)

  async function handleSearch() {
    setSrLoading(true)
    setSrError(null)
    setSrResults([])
    try {
      const results = await searchMbRelease({ artist: srArtist, album: srAlbum })
      setSrResults(results)
    } catch (e) {
      setSrError(e instanceof Error ? e.message : 'Search failed.')
    } finally {
      setSrLoading(false)
      setSrSearched(true)
    }
  }

  async function handleRidFetch() {
    const id = ridValue.trim()
    if (!id) return
    setRidLoading(true)
    setRidError(null)
    setRidRelease(null)
    try {
      const release = await getMbRelease(id)
      setRidRelease(release)
    } catch (e) {
      setRidError(e instanceof Error ? e.message : 'Fetch failed.')
    } finally {
      setRidLoading(false)
    }
  }

  async function applyRelease(releaseId: string) {
    setApplying(releaseId)
    setApplyError(null)
    try {
      const full = await getMbRelease(releaseId)

      // Build album-scope tag patch
      const albumPatch: Record<string, string> = {}
      const summaryFields: Record<string, string | number> = {
        album: full.album,
        albumartist: full.albumartist,
        date: full.date,
        label: full.label,
        catalognumber: full.catalognumber,
        totaltracks: full.totaltracks,
        totaldiscs: full.totaldiscs,
        releasestatus: full.status,
        musicbrainz_releaseid: full.mb_release_id,
      }
      for (const [k, v] of Object.entries(summaryFields)) {
        const s = String(v ?? '')
        if (s && s !== '0') albumPatch[k] = s
      }

      // Build a flat track-number → release track map (disc position + track info)
      const releaseTrackByNum: Record<string, { number: string; recording_id: string; disc: number }> = {}
      for (const disc of full.discs) {
        for (const t of disc.tracks) {
          releaseTrackByNum[t.number] = { number: t.number, recording_id: t.recording_id, disc: disc.position }
        }
      }

      // Apply to every staged track
      const errors: string[] = []
      for (const track of tracks) {
        try {
          const existing = await getPendingTags(track.id)
          const base: Record<string, string> = Object.keys(existing).length > 0 ? existing : {}

          // Seed from track's existing tags if no pending copy yet
          if (Object.keys(base).length === 0) {
            for (const [k, v] of Object.entries(track.tags as Record<string, unknown>)) {
              if (typeof v === 'string' && v) base[k] = v
            }
          }

          // Merge album-scope fields
          const merged = { ...base, ...albumPatch }

          // Match to release track by tracknumber and apply track-specific fields
          const trackNum = getTrackNumber(track)
          if (trackNum && releaseTrackByNum[trackNum]) {
            const rt = releaseTrackByNum[trackNum]
            if (rt.recording_id) merged['musicbrainz_trackid'] = rt.recording_id
            merged['discnumber'] = String(rt.disc)
          }

          await setPendingTags(track.id, merged)
        } catch (e) {
          errors.push(e instanceof Error ? e.message : 'unknown')
        }
      }

      if (errors.length > 0) {
        setApplyError(`${errors.length} track(s) failed: ${errors[0]}`)
      } else {
        onApplied()
      }
    } catch (e) {
      setApplyError(e instanceof Error ? e.message : 'Apply failed.')
    } finally {
      setApplying(null)
    }
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: 'search', label: 'Search' },
    { id: 'release-id', label: 'By Release ID' },
  ]

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="bg-bg-surface border border-border rounded w-[560px] flex flex-col"
        style={{ maxHeight: 'calc(100vh - 4rem)', maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
          <span className="text-text-primary text-sm font-semibold">
            Search Album — {tracks.length} track{tracks.length !== 1 ? 's' : ''}
          </span>
          <button onClick={onClose} className="text-text-muted hover:text-text-primary text-sm leading-none" aria-label="Close">
            ×
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-border flex-shrink-0">
          {tabs.map(t => (
            <button key={t.id} onClick={() => setActiveTab(t.id)}
              className={`text-xs px-4 py-2 border-b-2 transition-colors ${
                activeTab === t.id
                  ? 'text-accent border-accent'
                  : 'text-text-muted border-transparent hover:text-text-secondary'
              }`}>
              {t.label}
            </button>
          ))}
        </div>

        <div className="flex flex-col gap-3 px-4 py-4 overflow-y-auto flex-1">
          {applyError && <p className="text-destructive text-xs">{applyError}</p>}

          {/* Search tab */}
          {activeTab === 'search' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Artist</span>
                  <input type="text" value={srArtist} onChange={e => setSrArtist(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleSearch()}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Album</span>
                  <input type="text" value={srAlbum} onChange={e => setSrAlbum(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleSearch()}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <div className="flex justify-end">
                  <button onClick={handleSearch} disabled={srLoading}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50">
                    {srLoading ? 'Searching…' : 'Search'}
                  </button>
                </div>
              </div>

              {srError && <p className="text-destructive text-xs">{srError}</p>}

              {srResults.length > 0 && (
                <div className="flex flex-col gap-1">
                  <p className="text-text-muted text-xs uppercase tracking-wider">{srResults.length} result{srResults.length !== 1 ? 's' : ''}</p>
                  {srResults.map((r) => (
                    <ReleaseResultRow key={r.mb_release_id} release={r}
                      applying={applying === r.mb_release_id}
                      onApply={() => applyRelease(r.mb_release_id)} />
                  ))}
                </div>
              )}

              {srResults.length === 0 && !srLoading && !srError && (
                <p className="text-text-muted text-xs italic">
                  {srSearched ? 'No results found.' : 'Enter artist and/or album above and click Search.'}
                </p>
              )}
            </>
          )}

          {/* By Release ID tab */}
          {activeTab === 'release-id' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">MusicBrainz Release ID</span>
                  <input type="text" value={ridValue} onChange={e => setRidValue(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleRidFetch()}
                    placeholder="e.g. 3d4c3d43-a52f-4def-9e77-…"
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono" />
                </label>
                <div className="flex justify-end">
                  <button onClick={handleRidFetch} disabled={ridLoading || !ridValue.trim()}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50">
                    {ridLoading ? 'Fetching…' : 'Fetch'}
                  </button>
                </div>
              </div>

              {ridError && <p className="text-destructive text-xs">{ridError}</p>}

              {ridRelease && (
                <ReleaseResultRow release={ridRelease}
                  applying={applying === ridRelease.mb_release_id}
                  onApply={() => applyRelease(ridRelease.mb_release_id)} />
              )}

              {!ridRelease && !ridLoading && !ridError && (
                <p className="text-text-muted text-xs italic">Enter a MusicBrainz Release ID above and click Fetch.</p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}

function ReleaseResultRow({
  release,
  applying,
  onApply,
}: {
  release: MbReleaseSummary
  applying: boolean
  onApply: () => void
}) {
  return (
    <div className="flex items-start gap-2 px-2 py-1.5 bg-bg-panel border border-border rounded text-xs">
      {release.cover_art_url && (
        <img src={release.cover_art_url} alt="" className="w-10 h-10 object-cover rounded border border-border flex-shrink-0"
          onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none' }} />
      )}
      <div className="flex flex-col flex-1 min-w-0">
        <span className="text-text-primary font-medium truncate">{release.album}</span>
        <span className="text-text-muted truncate">{release.albumartist}{release.date ? ` · ${release.date}` : ''}</span>
        <span className="text-text-muted truncate">
          {release.totaltracks} tracks{release.totaldiscs > 1 ? `, ${release.totaldiscs} discs` : ''}
          {release.label ? ` · ${release.label}` : ''}
          {release.catalognumber ? ` · ${release.catalognumber}` : ''}
          {release.status ? ` · ${release.status}` : ''}
        </span>
      </div>
      <button onClick={onApply} disabled={applying}
        className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0 self-center">
        {applying ? '…' : 'Apply to All'}
      </button>
    </div>
  )
}

