import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { searchMb, searchFreedB, getMbRelease } from '../api/search'
import type { MbReleaseFull, MbReleaseTrack } from '../api/search'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import type { Track } from '../types/track'

type Tab = 'musicbrainz' | 'release-id' | 'freedb'

interface MbCandidate {
  tags: Record<string, string>
  confidence: number
}

interface FreedBCandidate {
  artist: string
  album: string
  year: string
  genre: string
  tracks: string[]
}

interface Props {
  track: Track
  onClose: () => void
}

export function IngestSearchDialog({ track, onClose }: Props) {
  const qc = useQueryClient()
  const [activeTab, setActiveTab] = useState<Tab>('musicbrainz')

  // MusicBrainz recording search tab state
  const [mbTitle, setMbTitle] = useState(track.title ?? '')
  const [mbArtist, setMbArtist] = useState(track.artist ?? '')
  const [mbAlbum, setMbAlbum] = useState(track.album ?? '')
  const [mbResults, setMbResults] = useState<MbCandidate[]>([])
  const [mbLoading, setMbLoading] = useState(false)
  const [mbError, setMbError] = useState<string | null>(null)
  const [mbSelecting, setMbSelecting] = useState<number | null>(null)
  const [mbSearched, setMbSearched] = useState(false)

  // Release ID tab state
  const [ridValue, setRidValue] = useState('')
  const [ridRelease, setRidRelease] = useState<MbReleaseFull | null>(null)
  const [ridLoading, setRidLoading] = useState(false)
  const [ridError, setRidError] = useState<string | null>(null)
  const [ridSelecting, setRidSelecting] = useState<string | null>(null)

  // FreeDB tab state
  const [fdbDiscId, setFdbDiscId] = useState(
    typeof track.tags?.DISCID === 'string' ? track.tags.DISCID : '',
  )
  const [fdbArtist, setFdbArtist] = useState(track.artist ?? '')
  const [fdbAlbum, setFdbAlbum] = useState(track.album ?? '')
  const [fdbResults, setFdbResults] = useState<FreedBCandidate[]>([])
  const [fdbLoading, setFdbLoading] = useState(false)
  const [fdbError, setFdbError] = useState<string | null>(null)
  const [fdbSelecting, setFdbSelecting] = useState<number | null>(null)
  const [fdbSearched, setFdbSearched] = useState(false)

  async function handleMbSearch() {
    setMbLoading(true)
    setMbError(null)
    setMbResults([])
    try {
      const results = await searchMb({ title: mbTitle, artist: mbArtist, album: mbAlbum })
      setMbResults(results)
    } catch (e) {
      setMbError(e instanceof Error ? e.message : 'Search failed.')
    } finally {
      setMbLoading(false)
      setMbSearched(true)
    }
  }

  async function handleMbSelect(idx: number, candidate: MbCandidate) {
    setMbSelecting(idx)
    try {
      await tagSuggestionsApi.create({
        track_id: track.id,
        source: 'mb_search',
        suggested_tags: candidate.tags,
        confidence: candidate.confidence,
      })
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      onClose()
    } catch (e) {
      setMbError(e instanceof Error ? e.message : 'Failed to create suggestion.')
    } finally {
      setMbSelecting(null)
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

  async function handleRidSelect(releaseTrack: MbReleaseTrack, discPosition: number) {
    if (!ridRelease) return
    const key = `${discPosition}-${releaseTrack.number}`
    setRidSelecting(key)
    try {
      const tags: Record<string, string> = {
        album: ridRelease.album,
        albumartist: ridRelease.albumartist,
        date: ridRelease.date,
        label: ridRelease.label,
        catalognumber: ridRelease.catalognumber,
        totaltracks: String(ridRelease.totaltracks),
        totaldiscs: String(ridRelease.totaldiscs),
        tracknumber: releaseTrack.number,
        discnumber: String(discPosition),
        musicbrainz_releaseid: ridRelease.mb_release_id,
        musicbrainz_trackid: releaseTrack.recording_id,
      }
      // Strip empty values
      for (const k of Object.keys(tags)) {
        if (!tags[k]) delete tags[k]
      }
      await tagSuggestionsApi.create({
        track_id: track.id,
        source: 'mb_search',
        suggested_tags: tags,
        confidence: 0.6,
      })
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      onClose()
    } catch (e) {
      setRidError(e instanceof Error ? e.message : 'Failed to create suggestion.')
    } finally {
      setRidSelecting(null)
    }
  }

  async function handleFdbSearch() {
    setFdbLoading(true)
    setFdbError(null)
    setFdbResults([])
    try {
      const results = await searchFreedB({
        disc_id: fdbDiscId.trim() || undefined,
        artist: fdbArtist.trim() || undefined,
        album: fdbAlbum.trim() || undefined,
      })
      setFdbResults(results)
    } catch (e) {
      setFdbError(e instanceof Error ? e.message : 'Search failed.')
    } finally {
      setFdbLoading(false)
      setFdbSearched(true)
    }
  }

  async function handleFdbSelect(idx: number, candidate: FreedBCandidate) {
    setFdbSelecting(idx)
    try {
      await tagSuggestionsApi.create({
        track_id: track.id,
        source: 'freedb',
        suggested_tags: {
          artist: candidate.artist,
          album: candidate.album,
          date: candidate.year,
          genre: candidate.genre,
        },
        confidence: 0.5,
      })
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      onClose()
    } catch (e) {
      setFdbError(e instanceof Error ? e.message : 'Failed to create suggestion.')
    } finally {
      setFdbSelecting(null)
    }
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: 'musicbrainz', label: 'MusicBrainz' },
    { id: 'release-id', label: 'By Release ID' },
    { id: 'freedb', label: 'FreeDB' },
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
            Search — {track.title ?? track.relative_path.split('/').pop()}
          </span>
          <button
            onClick={onClose}
            className="text-text-muted hover:text-text-primary text-sm leading-none"
            aria-label="Close"
          >
            ×
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-border flex-shrink-0">
          {tabs.map(t => (
            <button
              key={t.id}
              onClick={() => setActiveTab(t.id)}
              className={`text-xs px-4 py-2 border-b-2 transition-colors ${
                activeTab === t.id
                  ? 'text-accent border-accent'
                  : 'text-text-muted border-transparent hover:text-text-secondary'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        <div className="flex flex-col gap-3 px-4 py-4 overflow-y-auto flex-1">
          {/* MusicBrainz recording search tab */}
          {activeTab === 'musicbrainz' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Title</span>
                  <input type="text" value={mbTitle} onChange={e => setMbTitle(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleMbSearch()}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Artist</span>
                  <input type="text" value={mbArtist} onChange={e => setMbArtist(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleMbSearch()}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Album</span>
                  <input type="text" value={mbAlbum} onChange={e => setMbAlbum(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && handleMbSearch()}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <div className="flex justify-end">
                  <button onClick={handleMbSearch} disabled={mbLoading}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50">
                    {mbLoading ? 'Searching…' : 'Search'}
                  </button>
                </div>
              </div>
              {mbError && <p className="text-destructive text-xs">{mbError}</p>}
              {mbResults.length > 0 && (
                <div className="flex flex-col gap-1">
                  <p className="text-text-muted text-xs uppercase tracking-wider">{mbResults.length} result{mbResults.length !== 1 ? 's' : ''}</p>
                  {mbResults.map((c, i) => {
                    const pct = Math.round(c.confidence * 100)
                    return (
                      <div key={i} className="flex items-center gap-2 px-2 py-1.5 bg-bg-panel border border-border rounded text-xs">
                        <div className="flex flex-col flex-1 min-w-0">
                          <span className="text-text-primary font-medium truncate">
                            {c.tags.tracknumber && <span className="text-text-muted mr-1">#{c.tags.tracknumber}</span>}
                            {c.tags.title ?? '—'}
                          </span>
                          <span className="text-text-muted truncate">{c.tags.artist ?? '—'} · {c.tags.album ?? '—'}</span>
                          {c.tags.date && <span className="text-text-muted">{c.tags.date}</span>}
                        </div>
                        <span className={`text-[10px] font-mono shrink-0 ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>{pct}%</span>
                        <button onClick={() => handleMbSelect(i, c)} disabled={mbSelecting === i}
                          className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0">
                          {mbSelecting === i ? '…' : 'Select'}
                        </button>
                      </div>
                    )
                  })}
                </div>
              )}
              {mbResults.length === 0 && !mbLoading && !mbError && (
                <p className="text-text-muted text-xs italic">
                  {mbSearched ? 'No results found.' : 'Enter search terms above and click Search.'}
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
                <div className="flex flex-col gap-2">
                  <div className="px-2 py-1.5 bg-bg-panel border border-border rounded text-xs">
                    <div className="text-text-primary font-medium">{ridRelease.album}</div>
                    <div className="text-text-muted">{ridRelease.albumartist}{ridRelease.date ? ` · ${ridRelease.date}` : ''}{ridRelease.label ? ` · ${ridRelease.label}` : ''}</div>
                    <div className="text-text-muted">{ridRelease.totaltracks} tracks{ridRelease.totaldiscs > 1 ? `, ${ridRelease.totaldiscs} discs` : ''}{ridRelease.status ? ` · ${ridRelease.status}` : ''}</div>
                  </div>
                  <p className="text-text-muted text-xs uppercase tracking-wider">Pick the matching track</p>
                  {ridRelease.discs.map(disc => (
                    <div key={disc.position} className="flex flex-col gap-1">
                      {ridRelease.totaldiscs > 1 && (
                        <p className="text-text-muted text-[10px] font-mono uppercase">Disc {disc.position}</p>
                      )}
                      {disc.tracks.map(t => {
                        const key = `${disc.position}-${t.number}`
                        return (
                          <div key={key} className="flex items-center gap-2 px-2 py-1 bg-bg-panel border border-border rounded text-xs">
                            <span className="text-text-muted font-mono text-[10px] w-5 text-right shrink-0">{t.number}</span>
                            <span className="text-text-primary flex-1 truncate">—</span>
                            <button onClick={() => handleRidSelect(t, disc.position)} disabled={ridSelecting === key}
                              className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0">
                              {ridSelecting === key ? '…' : 'Select'}
                            </button>
                          </div>
                        )
                      })}
                    </div>
                  ))}
                </div>
              )}
              {!ridRelease && !ridLoading && !ridError && (
                <p className="text-text-muted text-xs italic">Enter a MusicBrainz Release ID above and click Fetch.</p>
              )}
            </>
          )}

          {/* FreeDB tab */}
          {activeTab === 'freedb' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Disc ID</span>
                  <input type="text" value={fdbDiscId} onChange={e => setFdbDiscId(e.target.value)}
                    placeholder="Optional — e.g. 8d0fce0b"
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono" />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Artist</span>
                  <input type="text" value={fdbArtist} onChange={e => setFdbArtist(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Album</span>
                  <input type="text" value={fdbAlbum} onChange={e => setFdbAlbum(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent" />
                </label>
                <div className="flex justify-end">
                  <button onClick={handleFdbSearch} disabled={fdbLoading}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50">
                    {fdbLoading ? 'Searching…' : 'Search'}
                  </button>
                </div>
              </div>
              {fdbError && <p className="text-destructive text-xs">{fdbError}</p>}
              {fdbResults.length > 0 && (
                <div className="flex flex-col gap-1">
                  <p className="text-text-muted text-xs uppercase tracking-wider">{fdbResults.length} result{fdbResults.length !== 1 ? 's' : ''}</p>
                  {fdbResults.map((c, i) => (
                    <div key={i} className="flex items-center gap-2 px-2 py-1.5 bg-bg-panel border border-border rounded text-xs">
                      <div className="flex flex-col flex-1 min-w-0">
                        <span className="text-text-primary font-medium truncate">{c.artist} — {c.album}</span>
                        <span className="text-text-muted">{c.year} · {c.genre} · {c.tracks.length} tracks</span>
                      </div>
                      <button onClick={() => handleFdbSelect(i, c)} disabled={fdbSelecting === i}
                        className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0">
                        {fdbSelecting === i ? '…' : 'Select'}
                      </button>
                    </div>
                  ))}
                </div>
              )}
              {fdbResults.length === 0 && !fdbLoading && !fdbError && (
                <p className="text-text-muted text-xs italic">
                  {fdbSearched ? 'No results found.' : 'Enter a Disc ID or artist/album and click Search.'}
                </p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}
