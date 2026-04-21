import { useState } from 'react'
import type { UpsertArtProfile } from '../types/artProfile'

interface Props {
  initial?: UpsertArtProfile
  onSave: (data: UpsertArtProfile) => Promise<void>
  onCancel: () => void
  isPending: boolean
}

export function ArtProfileForm({ initial, onSave, onCancel, isPending }: Props) {
  const [name, setName] = useState(initial?.name ?? '')
  const [format, setFormat] = useState<'jpeg' | 'png'>(initial?.format ?? 'jpeg')
  const [quality, setQuality] = useState(initial?.quality?.toString() ?? '85')
  const [maxWidth, setMaxWidth] = useState(initial?.max_width_px?.toString() ?? '')
  const [maxHeight, setMaxHeight] = useState(initial?.max_height_px?.toString() ?? '')
  const [maxSizeBytes, setMaxSizeBytes] = useState(initial?.max_size_bytes?.toString() ?? '')
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    const qualityNum = Number(quality)
    if (qualityNum < 1 || qualityNum > 100) {
      setError('Quality must be between 1 and 100.')
      return
    }
    const data: UpsertArtProfile = {
      name,
      format,
      quality: qualityNum,
      max_width_px: Number(maxWidth),
      max_height_px: Number(maxHeight),
      max_size_bytes: maxSizeBytes.trim() ? Number(maxSizeBytes) : undefined,
    }
    try {
      await onSave(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An unexpected error occurred.')
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-xs uppercase tracking-wider">Name</span>
        <input
          type="text" value={name} onChange={e => setName(e.target.value)} autoFocus required
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        />
      </label>

      <div className="flex gap-3">
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-xs uppercase tracking-wider">Format</span>
          <select
            value={format}
            onChange={e => setFormat(e.target.value as 'jpeg' | 'png')}
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          >
            <option value="jpeg">jpeg</option>
            <option value="png">png</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-xs uppercase tracking-wider">Quality (1–100)</span>
          <input
            type="number" value={quality} onChange={e => setQuality(e.target.value)}
            min={1} max={100} required
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
      </div>

      <div className="flex gap-3">
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-xs uppercase tracking-wider">Max Width (px)</span>
          <input
            type="number" value={maxWidth} onChange={e => setMaxWidth(e.target.value)}
            min={1} required
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-xs uppercase tracking-wider">Max Height (px)</span>
          <input
            type="number" value={maxHeight} onChange={e => setMaxHeight(e.target.value)}
            min={1} required
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
      </div>

      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-xs uppercase tracking-wider">Max Size (bytes, optional)</span>
        <input
          type="number" value={maxSizeBytes} onChange={e => setMaxSizeBytes(e.target.value)}
          placeholder="e.g. 524288"
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        />
      </label>

      {error && <p className="text-destructive text-xs">{error}</p>}

      <div className="flex justify-end gap-2 pt-1">
        <button
          type="button" onClick={onCancel}
          className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={!name.trim() || !maxWidth.trim() || !maxHeight.trim() || isPending}
          className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
        >
          {isPending ? 'Saving…' : 'Save Profile'}
        </button>
      </div>
    </form>
  )
}
