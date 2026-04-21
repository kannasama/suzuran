import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  createVirtualLibrary,
  updateVirtualLibrary,
  setSources,
} from '../api/virtualLibraries'
import { VirtualLibraryForm } from './VirtualLibraryForm'
import type { VirtualLibrary, UpsertVirtualLibrary } from '../types/virtualLibrary'

interface Props {
  /** When provided, modal is in edit mode */
  virtualLibrary?: VirtualLibrary
  onClose: () => void
}

export function VirtualLibraryFormModal({ virtualLibrary, onClose }: Props) {
  const qc = useQueryClient()
  const isEdit = virtualLibrary !== undefined

  const createMutation = useMutation({
    mutationFn: (data: UpsertVirtualLibrary) => createVirtualLibrary(data),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpsertVirtualLibrary }) =>
      updateVirtualLibrary(id, data),
  })

  const isPending = createMutation.isPending || updateMutation.isPending

  async function handleSave(
    data: UpsertVirtualLibrary,
    sources: Array<{ library_id: number; priority: number }>,
  ) {
    const sourcesWithProfile = sources.map(s => ({ ...s, library_profile_id: null }))
    if (isEdit) {
      await updateMutation.mutateAsync({ id: virtualLibrary!.id, data })
      await setSources(virtualLibrary!.id, sourcesWithProfile)
    } else {
      const created = await createMutation.mutateAsync(data)
      await setSources(created.id, sourcesWithProfile)
    }
    qc.invalidateQueries({ queryKey: ['virtual-libraries'] })
    onClose()
  }

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="bg-bg-surface border border-border rounded w-[520px] flex flex-col"
        style={{ maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <span className="text-text-primary text-sm font-semibold">
            {isEdit ? 'Edit Virtual Library' : 'New Virtual Library'}
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
        <div className="px-4 py-4">
          <VirtualLibraryForm
            initial={virtualLibrary}
            onSave={handleSave}
            onCancel={onClose}
            isPending={isPending}
          />
        </div>
      </div>
    </div>
  )
}
