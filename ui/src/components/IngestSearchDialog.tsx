import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { searchMb, searchFreedB } from '../api/search'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import type { Track } from '../types/track'

type Tab = 'musicbrainz' | 'freedb'

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

  // MusicBrainz tab state
  const [mbTitle, setMbTitle] = useState(track.title ?? '')
  const [mbArtist, setMbArtist] = useState(track.artist ?? '')
  const [mbAlbum, setMbAlbum] = useState(track.album ?? '')
  const [mbResults, setMbResults] = useState<MbCandidate[]>([])
  const [mbLoading, setMbLoading] = useState(false)
  const [mbError, setMbError] = useState<string | null>(null)
  const [mbSelecting, setMbSelecting] = useState<number | null>(null)

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
          <button
            onClick={() => setActiveTab('musicbrainz')}
            className={`text-xs px-4 py-2 border-b-2 transition-colors ${
              activeTab === 'musicbrainz'
                ? 'text-accent border-accent'
                : 'text-text-muted border-transparent hover:text-text-secondary'
            }`}
          >
            MusicBrainz
          </button>
          <button
            onClick={() => setActiveTab('freedb')}
            className={`text-xs px-4 py-2 border-b-2 transition-colors ${
              activeTab === 'freedb'
                ? 'text-accent border-accent'
                : 'text-text-muted border-transparent hover:text-text-secondary'
            }`}
          >
            FreeDB
          </button>
        </div>

        <div className="flex flex-col gap-3 px-4 py-4 overflow-y-auto flex-1">
          {/* MusicBrainz tab */}
          {activeTab === 'musicbrainz' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Title</span>
                  <input
                    type="text"
                    value={mbTitle}
                    onChange={e => setMbTitle(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Artist</span>
                  <input
                    type="text"
                    value={mbArtist}
                    onChange={e => setMbArtist(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Album</span>
                  <input
                    type="text"
                    value={mbAlbum}
                    onChange={e => setMbAlbum(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <div className="flex justify-end">
                  <button
                    onClick={handleMbSearch}
                    disabled={mbLoading}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
                  >
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
                          <span className="text-text-primary font-medium truncate">{c.tags.title ?? '—'}</span>
                          <span className="text-text-muted truncate">{c.tags.artist ?? '—'} · {c.tags.album ?? '—'}</span>
                          {c.tags.date && <span className="text-text-muted">{c.tags.date}</span>}
                        </div>
                        <span className={`text-[10px] font-mono shrink-0 ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}>
                          {pct}%
                        </span>
                        <button
                          onClick={() => handleMbSelect(i, c)}
                          disabled={mbSelecting === i}
                          className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0"
                        >
                          {mbSelecting === i ? '…' : 'Select'}
                        </button>
                      </div>
                    )
                  })}
                </div>
              )}

              {mbResults.length === 0 && !mbLoading && !mbError && (
                <p className="text-text-muted text-xs italic">Enter search terms above and click Search.</p>
              )}
            </>
          )}

          {/* FreeDB tab */}
          {activeTab === 'freedb' && (
            <>
              <div className="flex flex-col gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Disc ID</span>
                  <input
                    type="text"
                    value={fdbDiscId}
                    onChange={e => setFdbDiscId(e.target.value)}
                    placeholder="Optional — e.g. 8d0fce0b"
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Artist</span>
                  <input
                    type="text"
                    value={fdbArtist}
                    onChange={e => setFdbArtist(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Album</span>
                  <input
                    type="text"
                    value={fdbAlbum}
                    onChange={e => setFdbAlbum(e.target.value)}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <div className="flex justify-end">
                  <button
                    onClick={handleFdbSearch}
                    disabled={fdbLoading}
                    className="text-xs bg-accent text-bg-base rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
                  >
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
                      <button
                        onClick={() => handleFdbSelect(i, c)}
                        disabled={fdbSelecting === i}
                        className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 font-medium hover:opacity-90 disabled:opacity-50 shrink-0"
                      >
                        {fdbSelecting === i ? '…' : 'Select'}
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {fdbResults.length === 0 && !fdbLoading && !fdbError && (
                <p className="text-text-muted text-xs italic">Enter a Disc ID or artist/album and click Search.</p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}
