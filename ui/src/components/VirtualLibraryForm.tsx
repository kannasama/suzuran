import { useState, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { listLibraries } from '../api/libraries'
import { getSources } from '../api/virtualLibraries'
import { SourcePriorityList } from './SourcePriorityList'
import type { VirtualLibrary, UpsertVirtualLibrary } from '../types/virtualLibrary'

interface Props {
  initial?: VirtualLibrary
  onSave: (
    data: UpsertVirtualLibrary,
    sources: Array<{ library_id: number; priority: number }>,
  ) => Promise<void>
  onCancel: () => void
  isPending: boolean
}

export function VirtualLibraryForm({ initial, onSave, onCancel, isPending }: Props) {
  const [name, setName] = useState(initial?.name ?? '')
  const [rootPath, setRootPath] = useState(initial?.root_path ?? '')
  const [linkType, setLinkType] = useState<'symlink' | 'hardlink'>(initial?.link_type ?? 'symlink')
  const [sources, setSources] = useState<Array<{ library_id: number; priority: number }>>([])
  const [error, setError] = useState<string | null>(null)

  const { data: allLibraries = [] } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  // Load existing sources when editing
  useEffect(() => {
    if (initial?.id) {
      getSources(initial.id).then(fetched => {
        setSources(fetched.map(s => ({ library_id: s.library_id, priority: s.priority })))
      })
    }
  }, [initial?.id])

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    try {
      await onSave({ name, root_path: rootPath, link_type: linkType }, sources)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An unexpected error occurred.')
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Name</span>
        <input
          type="text"
          value={name}
          onChange={e => setName(e.target.value)}
          autoFocus
          required
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        />
      </label>

      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Root Path</span>
        <input
          type="text"
          value={rootPath}
          onChange={e => setRootPath(e.target.value)}
          required
          placeholder="/srv/virtual-library"
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
        />
      </label>

      <label className="flex flex-col gap-1">
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Link Type</span>
        <select
          value={linkType}
          onChange={e => setLinkType(e.target.value as 'symlink' | 'hardlink')}
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        >
          <option value="symlink">symlink</option>
          <option value="hardlink">hardlink</option>
        </select>
      </label>

      <div className="flex flex-col gap-1">
        <span className="text-text-muted text-[10px] uppercase tracking-wider">Source Libraries</span>
        <p className="text-text-muted text-[10px]">Priority 1 wins when the same track appears in multiple sources.</p>
        <SourcePriorityList
          sources={sources}
          allLibraries={allLibraries}
          onChange={setSources}
        />
      </div>

      {error && <p className="text-destructive text-xs">{error}</p>}

      <div className="flex justify-end gap-2 pt-1">
        <button
          type="button"
          onClick={onCancel}
          className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={!name.trim() || !rootPath.trim() || isPending}
          className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
        >
          {isPending ? 'Saving…' : 'Save'}
        </button>
      </div>
    </form>
  )
}
