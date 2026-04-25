import { useState, useRef, useCallback } from 'react'
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

  // Art state — initialise from existing suggestion url or empty
  const initialArtUrl = suggestion?.cover_art_url ?? ''
  const [artUrl, setArtUrl] = useState(initialArtUrl)
  const [artUploading, setArtUploading] = useState(false)
  const [artError, setArtError] = useState<string | null>(null)
  const [dragOver, setDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const uploadFile = useCallback(async (file: File) => {
    setArtUploading(true)
    setArtError(null)
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
        try { msg = JSON.parse(body).error ?? body } catch { /* raw text */ }
        throw new Error(msg)
      }
      const { url } = await resp.json()
      setArtUrl(url)
    } catch (err) {
      setArtError(err instanceof Error ? err.message : 'Upload failed')
    } finally {
      setArtUploading(false)
    }
  }, [])

  function handleDrop(e: React.DragEvent) {
    e.preventDefault()
    setDragOver(false)
    const file = e.dataTransfer.files[0]
    if (file) uploadFile(file)
  }

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
        ...(artUrl ? { cover_art_url: artUrl } : {}),
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

  // Displayed art: prefer new artUrl, fall back to embedded art on the track
  const displayArtSrc = artUrl || (track.has_embedded_art ? `/api/v1/tracks/${track.id}/art` : null)

  return (
    <div className="flex gap-0 bg-bg-panel border border-border rounded overflow-hidden">
      {/* ── Art zone ── */}
      <div className="flex-shrink-0 flex flex-col items-center justify-center w-[120px] min-h-[120px] p-2 gap-1.5">
        <div
          className={`relative w-[96px] h-[96px] rounded border-2 border-dashed flex items-center justify-center cursor-pointer transition-colors overflow-hidden
            ${dragOver ? 'border-accent bg-accent/10' : 'border-border hover:border-accent/60'}`}
          onDragOver={e => { e.preventDefault(); setDragOver(true) }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
          onClick={() => fileInputRef.current?.click()}
          title="Drop image or click to browse"
        >
          {displayArtSrc ? (
            <img
              src={displayArtSrc}
              alt="cover art"
              className="w-full h-full object-cover"
              onError={e => (e.currentTarget.style.display = 'none')}
            />
          ) : (
            <span className="text-[10px] text-text-muted/50 text-center leading-tight select-none">
              {artUploading ? 'Uploading…' : 'Drop art\nor click'}
            </span>
          )}
          {artUploading && (
            <div className="absolute inset-0 bg-bg-base/70 flex items-center justify-center">
              <span className="text-[10px] text-text-muted">…</span>
            </div>
          )}
        </div>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/jpeg,image/png,image/webp,image/gif"
          className="sr-only"
          onChange={e => { const f = e.target.files?.[0]; if (f) uploadFile(f); e.target.value = '' }}
        />
        {artUrl && (
          <button
            type="button"
            onClick={e => { e.stopPropagation(); setArtUrl('') }}
            className="text-[10px] text-text-muted hover:text-destructive"
          >
            Clear
          </button>
        )}
        {artError && <p className="text-[10px] text-destructive text-center">{artError}</p>}
      </div>

      {/* ── Divider ── */}
      <div className="w-px bg-border flex-shrink-0" />

      {/* ── Fields ── */}
      <div className="flex flex-col gap-2 p-3 flex-1 min-w-0">
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
    </div>
  )
}
