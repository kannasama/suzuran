import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import type { TagSuggestion, AlternativeRelease } from '../types/tagSuggestion'

export function AlternativesPanel({
  suggestion,
  onClose,
}: {
  suggestion: TagSuggestion
  onClose: () => void
}) {
  const qc = useQueryClient()
  const [pickingIdx, setPickingIdx] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  const alternatives: AlternativeRelease[] = suggestion.alternatives ?? []

  async function handlePick(alt: AlternativeRelease, idx: number) {
    setPickingIdx(idx)
    setError(null)
    try {
      await tagSuggestionsApi.create({
        track_id: suggestion.track_id,
        source: suggestion.source,
        suggested_tags: alt.suggested_tags,
        confidence: suggestion.confidence,
        cover_art_url: alt.cover_art_url,
        musicbrainz_release_id: alt.mb_release_id,
        musicbrainz_recording_id: suggestion.mb_recording_id,
      })
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to select release.')
    } finally {
      setPickingIdx(null)
    }
  }

  if (alternatives.length === 0) return null

  return (
    <div className="flex flex-col gap-1 p-2.5 bg-bg-panel border border-border rounded">
      <div className="flex items-center justify-between mb-0.5">
        <span className="text-[10px] uppercase tracking-wide text-text-muted font-mono">
          Alternative Releases ({alternatives.length})
        </span>
        <button
          type="button"
          onClick={onClose}
          className="text-text-muted text-xs hover:text-text-primary leading-none"
        >
          ✕
        </button>
      </div>
      {alternatives.map((alt, i) => (
        <div
          key={alt.mb_release_id}
          className="flex items-center gap-2 py-1 border-t border-border first:border-t-0"
        >
          {alt.cover_art_url && (
            <img
              src={alt.cover_art_url}
              alt=""
              className="w-7 h-7 object-cover rounded border border-border shrink-0"
              onError={e => {
                ;(e.currentTarget as HTMLImageElement).style.display = 'none'
              }}
            />
          )}
          <div className="flex flex-col min-w-0 flex-1">
            <span className="text-text-primary text-xs truncate">
              {alt.suggested_tags.album ?? '—'}
            </span>
            <span className="text-text-muted text-[10px] truncate">
              {[
                alt.suggested_tags.albumartist ?? alt.suggested_tags.artist,
                alt.suggested_tags.date,
              ]
                .filter(Boolean)
                .join(' · ')}
            </span>
          </div>
          <button
            type="button"
            onClick={() => handlePick(alt, i)}
            disabled={pickingIdx === i}
            className="text-xs bg-accent text-bg-base rounded px-2 py-0.5 hover:opacity-90 disabled:opacity-50 shrink-0"
          >
            {pickingIdx === i ? '…' : 'Use this'}
          </button>
        </div>
      ))}
      {error && <p className="text-destructive text-xs mt-1">{error}</p>}
    </div>
  )
}
