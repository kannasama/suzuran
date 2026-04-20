import { useState } from 'react'
import type { UpsertEncodingProfile } from '../types/encodingProfile'

const CODECS = ['aac', 'mp3', 'opus', 'flac', 'vorbis'] as const

interface Props {
  initial?: UpsertEncodingProfile
  onSave: (data: UpsertEncodingProfile) => Promise<void>
  onCancel: () => void
  isPending: boolean
}

export function EncodingProfileForm({ initial, onSave, onCancel, isPending }: Props) {
  const [name, setName] = useState(initial?.name ?? '')
  const [codec, setCodec] = useState<string>(initial?.codec ?? 'aac')
  const [bitrate, setBitrate] = useState(initial?.bitrate ?? '')
  const [sampleRate, setSampleRate] = useState(initial?.sample_rate?.toString() ?? '')
  const [channels, setChannels] = useState(initial?.channels?.toString() ?? '')
  const [advancedArgs, setAdvancedArgs] = useState(initial?.advanced_args ?? '')
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const isLossless = codec === 'flac'

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    const data: UpsertEncodingProfile = {
      name,
      codec,
      bitrate: isLossless ? undefined : (bitrate.trim() || undefined),
      sample_rate: sampleRate.trim() ? Number(sampleRate) : undefined,
      channels: channels.trim() ? Number(channels) : undefined,
      advanced_args: advancedArgs.trim() || undefined,
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
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Name</span>
        <input
          type="text" value={name} onChange={e => setName(e.target.value)} autoFocus required
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        />
      </label>

      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Codec</span>
        <select
          value={codec}
          onChange={e => setCodec(e.target.value)}
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        >
          {CODECS.map(c => (
            <option key={c} value={c}>{c}</option>
          ))}
        </select>
      </label>

      {!isLossless && (
        <label className="flex flex-col gap-1">
          <span className="text-text-muted text-[10px] uppercase tracking-wider">Bitrate</span>
          <input
            type="text" value={bitrate} onChange={e => setBitrate(e.target.value)}
            placeholder="e.g. 320k, 256k, 192k"
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
      )}

      <div className="flex gap-3">
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-[10px] uppercase tracking-wider">Sample Rate</span>
          <input
            type="number" value={sampleRate} onChange={e => setSampleRate(e.target.value)}
            placeholder="e.g. 44100"
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
        <label className="flex flex-col gap-1 flex-1">
          <span className="text-text-muted text-[10px] uppercase tracking-wider">Channels</span>
          <input
            type="number" value={channels} onChange={e => setChannels(e.target.value)}
            placeholder="e.g. 2"
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
          />
        </label>
      </div>

      <div>
        <button
          type="button"
          onClick={() => setShowAdvanced(v => !v)}
          className="text-text-muted text-[10px] uppercase tracking-wider hover:text-text-secondary"
        >
          {showAdvanced ? '▾' : '▸'} Advanced
        </button>
        {showAdvanced && (
          <label className="flex flex-col gap-1 mt-2">
            <span className="text-text-muted text-[10px] uppercase tracking-wider">Additional ffmpeg Args</span>
            <textarea
              value={advancedArgs} onChange={e => setAdvancedArgs(e.target.value)}
              rows={2}
              placeholder="e.g. -profile:a aac_low"
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono resize-y"
            />
          </label>
        )}
      </div>

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
          disabled={!name.trim() || isPending}
          className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
        >
          {isPending ? 'Saving…' : 'Save Profile'}
        </button>
      </div>
    </form>
  )
}
