import { useState } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { listLibraries } from '../api/libraries'
import { transcodeApi } from '../api/transcode'

interface Props {
  mode: 'track' | 'library'
  sourceId: number
  onClose: () => void
}

export function TranscodeDialog({ mode, sourceId, onClose }: Props) {
  const [targetLibraryId, setTargetLibraryId] = useState<number | null>(null)
  const [status, setStatus] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const { data: libraries = [], isLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  // All libraries are potential transcode targets (encoding profile is managed via library profiles)
  const targetLibraries = libraries

  const transcodeMutation = useMutation({
    mutationFn: (action: 'all' | 'sync') => {
      if (targetLibraryId == null) return Promise.reject(new Error('No target selected'))
      if (mode === 'track') {
        return transcodeApi.transcodeTrack(sourceId, targetLibraryId)
      }
      return action === 'sync'
        ? transcodeApi.transcodeSync(sourceId, targetLibraryId)
        : transcodeApi.transcodeLibrary(sourceId, targetLibraryId)
    },
    onSuccess: (res, action) => {
      if (mode === 'library' && res.data?.count != null) {
        setStatus(`Enqueued ${res.data.count} transcode job${res.data.count !== 1 ? 's' : ''} (${action === 'sync' ? 'sync' : 'all'})`)
      } else {
        setStatus('Transcode job enqueued.')
      }
      setError(null)
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'An error occurred.')
      setStatus(null)
    },
  })

  const selectedLib = targetLibraries.find(l => l.id === targetLibraryId)

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="bg-bg-surface border border-border rounded w-80 flex flex-col"
        style={{ maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <span className="text-text-primary text-sm font-semibold">
            {mode === 'track' ? 'Transcode Track' : 'Transcode Library'}
          </span>
          <button
            onClick={onClose}
            className="text-text-muted hover:text-text-primary text-sm leading-none"
            aria-label="Close"
          >
            ×
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-3 px-4 py-4">
          {isLoading && (
            <p className="text-text-muted text-xs">Loading libraries…</p>
          )}

          {!isLoading && targetLibraries.length === 0 && (
            <p className="text-text-muted text-xs">
              No libraries found. Create a target library first.
            </p>
          )}

          {!isLoading && targetLibraries.length > 0 && (
            <label className="flex flex-col gap-1">
              <span className="text-text-muted text-xs uppercase tracking-wider">
                Target Library
              </span>
              <select
                value={targetLibraryId ?? ''}
                onChange={e => setTargetLibraryId(e.target.value === '' ? null : Number(e.target.value))}
                className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
              >
                <option value="">Select…</option>
                {targetLibraries.map(l => (
                  <option key={l.id} value={l.id}>
                    {l.name} ({l.format.toUpperCase()})
                  </option>
                ))}
              </select>
              {selectedLib && (
                <span className="text-text-muted text-xs font-mono truncate">
                  {selectedLib.root_path}
                </span>
              )}
            </label>
          )}

          {error && <p className="text-destructive text-xs">{error}</p>}
          {status && <p className="text-accent text-xs">{status}</p>}

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border"
            >
              {status ? 'Close' : 'Cancel'}
            </button>

            {!status && (
              <>
                {mode === 'library' && (
                  <button
                    type="button"
                    disabled={targetLibraryId == null || transcodeMutation.isPending}
                    onClick={() => transcodeMutation.mutate('sync')}
                    className="text-xs text-text-secondary bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    {transcodeMutation.isPending ? '…' : 'Sync missing'}
                  </button>
                )}
                <button
                  type="button"
                  disabled={targetLibraryId == null || transcodeMutation.isPending}
                  onClick={() => transcodeMutation.mutate('all')}
                  className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
                >
                  {transcodeMutation.isPending
                    ? '…'
                    : mode === 'track'
                    ? 'Transcode'
                    : 'Transcode all'}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
