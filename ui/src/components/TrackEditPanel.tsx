import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'

const EDIT_TAG_FIELDS = [
  { key: 'title',       label: 'Title'        },
  { key: 'artist',      label: 'Artist'       },
  { key: 'albumartist', label: 'Album Artist' },
  { key: 'album',       label: 'Album'        },
  { key: 'tracknumber', label: 'Track #'      },
  { key: 'date',        label: 'Date'         },
  { key: 'genre',       label: 'Genre'        },
] as const

export function TrackEditPanel({
  track,
  suggestion,
  onClose,
}: {
  track: Track
  suggestion: TagSuggestion | undefined
  onClose: () => void
}) {
  const qc = useQueryClient()

  const initialTags = suggestion?.suggested_tags ?? {}
  const trackFallback: Record<string, string> = {
    title:       track.title       ?? '',
    artist:      track.artist      ?? '',
    albumartist: track.albumartist ?? '',
    album:       track.album       ?? '',
    tracknumber: track.tracknumber ?? '',
    date:        track.date        ?? '',
    genre:       track.genre       ?? '',
  }

  const [fields, setFields] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      EDIT_TAG_FIELDS.map(({ key }) => [key, initialTags[key] ?? trackFallback[key] ?? ''])
    )
  )
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  async function handleSave() {
    setSaving(true)
    setSaveError(null)
    try {
      const tags: Record<string, string> = {}
      for (const { key } of EDIT_TAG_FIELDS) {
        if (fields[key].trim() !== '') tags[key] = fields[key].trim()
      }
      await tagSuggestionsApi.create({
        track_id: track.id,
        // 'mb_search' is the closest valid source for manually-entered tags
        // (backend schema has no 'manual' source yet)
        source: 'mb_search',
        suggested_tags: tags,
        confidence: 1.0,
      })
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] })
      qc.invalidateQueries({ queryKey: ['inbox-count'] })
      onClose()
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save.')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex flex-col gap-2 p-3 bg-bg-panel border border-border rounded">
      <div className="grid grid-cols-2 gap-x-3 gap-y-1.5">
        {EDIT_TAG_FIELDS.map(({ key, label }) => (
          <label key={key} className="flex flex-col gap-0.5">
            <span className="text-text-muted text-[10px] uppercase tracking-wider">{label}</span>
            <input
              type="text"
              value={fields[key]}
              onChange={e => setFields(prev => ({ ...prev, [key]: e.target.value }))}
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent"
            />
          </label>
        ))}
      </div>
      {saveError && <p className="text-destructive text-xs">{saveError}</p>}
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onClose}
          className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
        >
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  )
}
