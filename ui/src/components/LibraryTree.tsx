import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listLibraries, deleteLibrary, type Library } from '../api/libraries'
import { LibraryFormModal } from './LibraryFormModal'

interface Props {
  isAdmin: boolean
  selectedLibraryId: number | null
  onSelectLibrary: (id: number) => void
}

export function LibraryTree({ isAdmin, selectedLibraryId, onSelectLibrary }: Props) {
  const queryClient = useQueryClient()

  const { data: libraries = [], isLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  const [showCreateModal, setShowCreateModal] = useState(false)
  const [editingLibrary, setEditingLibrary] = useState<Library | null>(null)

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteLibrary(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
    },
  })

  function handleDelete(lib: Library) {
    if (!window.confirm(`Delete library "${lib.name}"? This cannot be undone.`)) return
    deleteMutation.mutate(lib.id)
  }

  if (isLoading) {
    return <div className="p-3 text-text-muted text-xs">Loading…</div>
  }

  // Split into roots and children
  const roots = libraries.filter(l => l.parent_library_id === null)
  const childrenOf = (parentId: number) =>
    libraries.filter(l => l.parent_library_id === parentId)

  const isEmpty = libraries.length === 0

  return (
    <>
      <div className="flex flex-col overflow-y-auto text-xs">
        {/* Header row */}
        <div className="px-2 py-1 mb-0 border-b border-border-subtle flex items-center gap-1">
          <span className="text-text-muted uppercase text-[9px] tracking-wider flex-1">Libraries</span>
          {isAdmin && (
            <button
              onClick={() => setShowCreateModal(true)}
              title="Add library"
              className="text-text-muted hover:text-accent leading-none px-0.5"
            >
              +
            </button>
          )}
        </div>

        {/* Empty state */}
        {isEmpty && (
          <div className="px-2 py-3 text-text-muted text-[10px]">
            No libraries.
            {isAdmin && (
              <>
                {' '}
                <button
                  onClick={() => setShowCreateModal(true)}
                  className="text-accent hover:underline"
                >
                  Add your first library.
                </button>
              </>
            )}
          </div>
        )}

        {/* Tree: roots, then their children indented */}
        {roots.map(root => (
          <div key={root.id}>
            <LibraryRow
              library={root}
              isSelected={selectedLibraryId === root.id}
              isAdmin={isAdmin}
              onSelect={() => onSelectLibrary(root.id)}
              onEdit={() => setEditingLibrary(root)}
              onDelete={() => handleDelete(root)}
            />
            {childrenOf(root.id).map(child => (
              <LibraryRow
                key={child.id}
                library={child}
                isSelected={selectedLibraryId === child.id}
                isAdmin={isAdmin}
                onSelect={() => onSelectLibrary(child.id)}
                onEdit={() => setEditingLibrary(child)}
                onDelete={() => handleDelete(child)}
                indent
              />
            ))}
          </div>
        ))}
      </div>

      {/* Modals */}
      {showCreateModal && (
        <LibraryFormModal
          libraries={libraries}
          onClose={() => setShowCreateModal(false)}
        />
      )}
      {editingLibrary && (
        <LibraryFormModal
          library={editingLibrary}
          libraries={libraries}
          onClose={() => setEditingLibrary(null)}
        />
      )}
    </>
  )
}

interface RowProps {
  library: Library
  isSelected: boolean
  isAdmin: boolean
  onSelect: () => void
  onEdit: () => void
  onDelete: () => void
  indent?: boolean
}

function LibraryRow({ library, isSelected, isAdmin, onSelect, onEdit, onDelete, indent }: RowProps) {
  return (
    <div
      className={`group flex items-center gap-1 pr-1 cursor-pointer ${
        indent ? 'pl-4' : 'pl-2'
      } py-0.5 ${
        isSelected
          ? 'bg-accent-muted border-l-2 border-accent text-accent'
          : 'text-text-secondary hover:bg-bg-hover border-l-2 border-transparent'
      }`}
      onClick={onSelect}
    >
      <span className="flex-1 truncate">{library.name}</span>
      <span className="text-text-muted uppercase text-[9px] tracking-wider shrink-0">
        {library.format}
      </span>
      {isAdmin && (
        <span className="hidden group-hover:flex items-center gap-0.5 shrink-0">
          <button
            onClick={e => { e.stopPropagation(); onEdit() }}
            title="Edit"
            className="text-text-muted hover:text-text-primary px-0.5 leading-none"
          >
            ✎
          </button>
          <button
            onClick={e => { e.stopPropagation(); onDelete() }}
            title="Delete"
            className="text-text-muted hover:text-destructive px-0.5 leading-none"
          >
            ×
          </button>
        </span>
      )}
    </div>
  )
}
