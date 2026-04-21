import { useState } from 'react'

interface Props {
  value: string
  onChange: (url: string) => void
}

export function ImageUpload({ value, onChange }: Props) {
  const [uploading, setUploading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setUploading(true)
    setError(null)
    try {
      // Strip non-ASCII filename — multer fails to parse Content-Disposition
      // headers containing raw multibyte characters. The backend uses a UUID
      // for storage so the original filename is irrelevant.
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
        try { msg = JSON.parse(body).error ?? body } catch { /* use raw text */ }
        throw new Error(msg)
      }
      const { url } = await resp.json()
      onChange(url)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'upload failed')
    } finally {
      setUploading(false)
      e.target.value = ''
    }
  }

  return (
    <div className="space-y-2">
      <label className="block text-xs text-text-muted">Background image</label>
      <div className="flex gap-2 items-center">
        <input
          type="text"
          placeholder="https://… or upload a file"
          value={value}
          onChange={e => onChange(e.target.value)}
          className="flex-1 text-sm bg-bg-panel border border-border text-text-primary rounded px-2 py-1"
        />
        {value && (
          <button
            onClick={() => onChange('')}
            className="text-xs text-text-muted hover:text-destructive"
          >
            Clear
          </button>
        )}
      </div>
      <div className="flex items-center gap-3">
        <label className="cursor-pointer text-xs text-accent hover:underline">
          {uploading ? 'Uploading…' : 'Upload file…'}
          <input
            type="file"
            accept="image/jpeg,image/png,image/webp,image/gif"
            className="sr-only"
            onChange={handleFile}
            disabled={uploading}
          />
        </label>
        {value && (
          <img
            src={value}
            alt="preview"
            className="h-8 w-8 rounded object-cover border border-border"
            onError={e => (e.currentTarget.style.display = 'none')}
          />
        )}
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  )
}
