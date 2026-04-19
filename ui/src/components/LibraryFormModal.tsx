import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  createLibrary,
  updateLibrary,
  type Library,
  type CreateLibraryInput,
  type UpdateLibraryInput,
} from '../api/libraries'

interface Props {
  /** When provided, the modal is in edit mode for this library */
  library?: Library
  /** All libraries, used to populate the parent selector in create mode */
  libraries: Library[]
  onClose: () => void
}

const FORMATS = ['flac', 'aac', 'mp3', 'opus', 'wav'] as const

export function LibraryFormModal({ library, libraries, onClose }: Props) {
  const isEdit = library !== undefined
  const queryClient = useQueryClient()

  // Create mode state
  const [name, setName] = useState(library?.name ?? '')
  const [rootPath, setRootPath] = useState('')
  const [format, setFormat] = useState<string>('flac')
  const [parentId, setParentId] = useState<number | null>(null)

  const [error, setError] = useState<string | null>(null)

  const createMutation = useMutation({
    mutationFn: (input: CreateLibraryInput) => createLibrary(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
      onClose()
    },
    onError: (err: unknown) => {
      if (err instanceof Error) {
        setError(err.message)
      } else {
        setError('An unexpected error occurred.')
      }
    },
  })

  const updateMutation = useMutation({
    mutationFn: (input: UpdateLibraryInput) => updateLibrary(library!.id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
      onClose()
    },
    onError: (err: unknown) => {
      if (err instanceof Error) {
        setError(err.message)
      } else {
        setError('An unexpected error occurred.')
      }
    },
  })

  const isPending = createMutation.isPending || updateMutation.isPending

  const isSubmitDisabled =
    isPending ||
    name.trim() === '' ||
    (!isEdit && rootPath.trim() === '')

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)

    if (isEdit) {
      updateMutation.mutate({
        name: name.trim(),
        scan_enabled: library!.scan_enabled,
        scan_interval_secs: library!.scan_interval_secs,
        auto_transcode_on_ingest: library!.auto_transcode_on_ingest,
        auto_organize_on_ingest: library!.auto_organize_on_ingest,
      })
    } else {
      createMutation.mutate({
        name: name.trim(),
        root_path: rootPath.trim(),
        format,
        parent_library_id: parentId,
      })
    }
  }

  // Candidate parents: all libraries that are themselves roots (no parent), excluding
  // the library being edited
  const parentCandidates = libraries.filter(
    l => l.parent_library_id === null && l.id !== library?.id,
  )

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="bg-bg-surface border border-border rounded w-96 flex flex-col"
        style={{ maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <span className="text-text-primary text-sm font-semibold">
            {isEdit ? 'Edit Library' : 'Add Library'}
          </span>
          <button
            onClick={onClose}
            className="text-text-muted hover:text-text-primary text-sm leading-none"
            aria-label="Close"
          >
            ×
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="flex flex-col gap-3 px-4 py-4">
          {/* Name */}
          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-[10px] uppercase tracking-wider">Name</span>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="My Music"
              autoFocus
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            />
          </label>

          {/* Create-only fields */}
          {!isEdit && (
            <>
              {/* Root Path */}
              <label className="flex flex-col gap-1">
                <span className="text-text-muted text-[10px] uppercase tracking-wider">Root Path</span>
                <input
                  type="text"
                  value={rootPath}
                  onChange={e => setRootPath(e.target.value)}
                  placeholder="/mnt/music/flac"
                  className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
                />
              </label>

              {/* Format */}
              <label className="flex flex-col gap-1">
                <span className="text-text-muted text-[10px] uppercase tracking-wider">Format</span>
                <select
                  value={format}
                  onChange={e => setFormat(e.target.value)}
                  className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                >
                  {FORMATS.map(f => (
                    <option key={f} value={f}>{f.toUpperCase()}</option>
                  ))}
                </select>
              </label>

              {/* Parent Library */}
              <label className="flex flex-col gap-1">
                <span className="text-text-muted text-[10px] uppercase tracking-wider">Parent Library</span>
                <select
                  value={parentId ?? ''}
                  onChange={e => setParentId(e.target.value === '' ? null : Number(e.target.value))}
                  className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                >
                  <option value="">None</option>
                  {parentCandidates.map(l => (
                    <option key={l.id} value={l.id}>{l.name}</option>
                  ))}
                </select>
              </label>
            </>
          )}

          {/* Inline error */}
          {error && (
            <p className="text-destructive text-xs">{error}</p>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitDisabled}
              className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
            >
              {isPending ? 'Saving…' : isEdit ? 'Save' : 'Add Library'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
