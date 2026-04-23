import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { tagSuggestionsApi } from '../api/tagSuggestions'
import type { Track } from '../types/track'
import type { TagSuggestion } from '../types/tagSuggestion'

interface TagField {
  key: string
  label: string
  cols?: number
}

const COL_SPAN: Record<number, string> = {
  1: 'col-span-1',
  2: 'col-span-2',
  3: 'col-span-3',
  4: 'col-span-4',
  6: 'col-span-6',
}

const EDIT_TAG_FIELDS: TagField[] = [
  // ── Basic ───────────────────────────────────────────────────────────────────
  { key: 'title',                      label: 'Title',                 cols: 4 },
  { key: 'date',                       label: 'Date',                  cols: 2 },
  { key: 'artist',                     label: 'Artist',                cols: 3 },
  { key: 'albumartist',                label: 'Album Artist',          cols: 3 },
  { key: 'album',                      label: 'Album',                 cols: 4 },
  { key: 'genre',                      label: 'Genre',                 cols: 2 },
  // ── Track / disc ────────────────────────────────────────────────────────────
  { key: 'tracknumber',                label: 'Track #',               cols: 1 },
  { key: 'totaltracks',                label: 'Total Tracks',          cols: 1 },
  { key: 'discnumber',                 label: 'Disc #',                cols: 1 },
  { key: 'totaldiscs',                 label: 'Total Discs',           cols: 1 },
  { key: 'releasecountry',             label: 'Release Country',       cols: 1 },
  { key: 'originalyear',               label: 'Original Year',         cols: 1 },
  // ── Sort ────────────────────────────────────────────────────────────────────
  { key: 'albumartistsort',            label: 'Album Artist Sort',     cols: 3 },
  { key: 'artistsort',                 label: 'Artist Sort',           cols: 3 },
  // ── Release metadata ────────────────────────────────────────────────────────
  { key: 'releasetype',                label: 'Release Type',          cols: 2 },
  { key: 'releasestatus',              label: 'Release Status',        cols: 2 },
  { key: 'originaldate',               label: 'Original Release Date', cols: 2 },
  // ── Label / commercial ──────────────────────────────────────────────────────
  { key: 'label',                      label: 'Record Label',          cols: 3 },
  { key: 'catalognumber',              label: 'Catalog #',             cols: 2 },
  { key: 'barcode',                    label: 'Barcode',               cols: 1 },
  // ── MusicBrainz IDs ─────────────────────────────────────────────────────────
  { key: 'musicbrainz_artistid',       label: 'MB Artist ID',          cols: 6 },
  { key: 'musicbrainz_albumartistid',  label: 'MB Release Artist ID',  cols: 6 },
  { key: 'musicbrainz_releasegroupid', label: 'MB Release Group ID',   cols: 6 },
  { key: 'musicbrainz_releaseid',      label: 'MB Release ID',         cols: 6 },
  { key: 'musicbrainz_trackid',        label: 'MB Recording ID',       cols: 6 },
]

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
      <div className="grid grid-cols-6 gap-x-3 gap-y-1.5">
        {EDIT_TAG_FIELDS.map(({ key, label, cols }) => (
          <label
            key={key}
            className={`flex flex-col gap-0.5 ${COL_SPAN[cols ?? 2] ?? 'col-span-2'}`}
          >
            <span className="text-text-muted text-[10px] uppercase tracking-wider">{label}</span>
            <input
              type="text"
              value={fields[key]}
              onChange={e => setFields(prev => ({ ...prev, [key]: e.target.value }))}
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent font-mono"
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
