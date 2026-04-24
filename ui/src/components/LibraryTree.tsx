import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listLibraries, deleteLibrary, type Library } from '../api/libraries'
import { listVirtualLibraries, deleteVirtualLibrary, triggerSync } from '../api/virtualLibraries'
import type { VirtualLibrary } from '../types/virtualLibrary'
import { LibraryFormModal } from './LibraryFormModal'
import { VirtualLibraryFormModal } from './VirtualLibraryFormModal'

export type BrowseMode = 'artist' | 'albumartist' | 'album' | 'genre'

const BROWSE_OPTIONS: { key: BrowseMode; label: string }[] = [
  { key: 'artist',      label: 'Artist' },
  { key: 'albumartist', label: 'Album Artist' },
  { key: 'album',       label: 'Album' },
  { key: 'genre',       label: 'Genre' },
]

interface Props {
  isAdmin: boolean
  isLibraryAdmin: boolean
  selectedLibraryId: number | null
  onSelectLibrary: (id: number) => void
  selectedVirtualLibraryId: number | null
  onSelectVirtualLibrary: (id: number) => void
  selectedBrowseMode: BrowseMode | null
  onSelectBrowseMode: (libraryId: number, mode: BrowseMode | null) => void
}

export function LibraryTree({
  isAdmin,
  isLibraryAdmin,
  selectedLibraryId,
  onSelectLibrary,
  selectedVirtualLibraryId,
  onSelectVirtualLibrary,
  selectedBrowseMode,
  onSelectBrowseMode,
}: Props) {
  const queryClient = useQueryClient()

  const { data: libraries = [], isLoading: libsLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  const { data: virtualLibraries = [] } = useQuery({
    queryKey: ['virtual-libraries'],
    queryFn: listVirtualLibraries,
    enabled: isLibraryAdmin,
  })

  const [showCreateModal, setShowCreateModal] = useState(false)
  const [editingLibrary, setEditingLibrary] = useState<Library | null>(null)
  const [showCreateVLibModal, setShowCreateVLibModal] = useState(false)
  const [editingVirtualLibrary, setEditingVirtualLibrary] = useState<VirtualLibrary | null>(null)
  const [syncingId, setSyncingId] = useState<number | null>(null)

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteLibrary(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
    },
  })

  const deleteVLibMutation = useMutation({
    mutationFn: (id: number) => deleteVirtualLibrary(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['virtual-libraries'] })
    },
  })

  async function handleSyncVLib(id: number) {
    setSyncingId(id)
    try {
      await triggerSync(id)
    } finally {
      setSyncingId(null)
    }
  }

  function handleDelete(lib: Library) {
    if (!window.confirm(`Delete library "${lib.name}"? This cannot be undone.`)) return
    deleteMutation.mutate(lib.id)
  }

  if (libsLoading) {
    return <div className="p-3 text-text-muted text-xs">Loading…</div>
  }

  return (
    <>
      <div className="flex flex-col overflow-y-auto text-xs">
        {/* Libraries header */}
        <div className="px-2 py-1 mb-0 border-b border-border-subtle flex items-center gap-1">
          <span className="text-text-muted uppercase text-[11px] tracking-wider flex-1">Libraries</span>
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
        {libraries.length === 0 && (
          <div className="px-2 py-3 text-text-muted text-xs">
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

        {/* Library list with browse sub-nodes */}
        {libraries.map(lib => (
          <div key={lib.id}>
            <LibraryRow
              library={lib}
              isSelected={selectedLibraryId === lib.id}
              isAdmin={isAdmin}
              onSelect={() => onSelectLibrary(lib.id)}
              onEdit={() => setEditingLibrary(lib)}
              onDelete={() => handleDelete(lib)}
            />
            {/* Browse sub-nodes — always visible */}
            <div className="flex flex-col">
              {/* All Tracks */}
              <button
                onClick={() => onSelectLibrary(lib.id)}
                className={`text-left pl-5 pr-2 py-0.5 text-[11px] truncate transition-colors ${
                  selectedLibraryId === lib.id && selectedBrowseMode === null
                    ? 'text-accent'
                    : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                All Tracks
              </button>
              {BROWSE_OPTIONS.map(opt => (
                <button
                  key={opt.key}
                  onClick={() => onSelectBrowseMode(lib.id, opt.key)}
                  className={`text-left pl-5 pr-2 py-0.5 text-[11px] truncate transition-colors ${
                    selectedLibraryId === lib.id && selectedBrowseMode === opt.key
                      ? 'text-accent'
                      : 'text-text-muted hover:text-text-secondary'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        ))}

        {/* Virtual Libraries section */}
        {isLibraryAdmin && (
          <>
            <div className="px-2 py-1 mt-1 border-b border-border-subtle border-t border-t-border-subtle flex items-center gap-1">
              <span className="text-text-muted uppercase text-[11px] tracking-wider flex-1">Virtual</span>
              <button
                onClick={() => setShowCreateVLibModal(true)}
                title="Add virtual library"
                className="text-text-muted hover:text-accent leading-none px-0.5"
              >
                +
              </button>
            </div>
            {virtualLibraries.length === 0 && (
              <div className="px-2 py-2 text-text-muted text-xs">
                <button
                  onClick={() => setShowCreateVLibModal(true)}
                  className="text-accent hover:underline"
                >
                  Add your first virtual library.
                </button>
              </div>
            )}
            {virtualLibraries.map(vl => (
              <VirtualLibraryRow
                key={vl.id}
                virtualLibrary={vl}
                isSelected={selectedVirtualLibraryId === vl.id}
                isSyncing={syncingId === vl.id}
                onSelect={() => onSelectVirtualLibrary(vl.id)}
                onEdit={() => setEditingVirtualLibrary(vl)}
                onDelete={() => {
                  if (window.confirm(`Delete virtual library "${vl.name}"?`)) {
                    deleteVLibMutation.mutate(vl.id)
                  }
                }}
                onSync={() => handleSyncVLib(vl.id)}
              />
            ))}
          </>
        )}
      </div>

      {/* Modals */}
      {showCreateModal && (
        <LibraryFormModal
          onClose={() => setShowCreateModal(false)}
        />
      )}
      {editingLibrary && (
        <LibraryFormModal
          library={editingLibrary}
          onClose={() => setEditingLibrary(null)}
        />
      )}
      {showCreateVLibModal && (
        <VirtualLibraryFormModal
          onClose={() => setShowCreateVLibModal(false)}
        />
      )}
      {editingVirtualLibrary && (
        <VirtualLibraryFormModal
          virtualLibrary={editingVirtualLibrary}
          onClose={() => setEditingVirtualLibrary(null)}
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

interface VirtualRowProps {
  virtualLibrary: VirtualLibrary
  isSelected: boolean
  isSyncing: boolean
  onSelect: () => void
  onEdit: () => void
  onDelete: () => void
  onSync: () => void
}

function VirtualLibraryRow({ virtualLibrary, isSelected, isSyncing, onSelect, onEdit, onDelete, onSync }: VirtualRowProps) {
  return (
    <div
      className={`group flex items-center gap-1 pl-2 pr-1 py-0.5 cursor-pointer border-l-2 ${
        isSelected
          ? 'bg-accent-muted border-accent text-accent'
          : 'text-text-secondary hover:bg-bg-hover border-transparent'
      }`}
      onClick={onSelect}
    >
      <span className="flex-1 truncate">{virtualLibrary.name}</span>
      <span className="text-text-muted uppercase text-[9px] tracking-wider shrink-0">
        {virtualLibrary.link_type === 'symlink' ? 'sym' : 'hard'}
      </span>
      <span className="hidden group-hover:flex items-center gap-0.5 shrink-0">
        <button
          onClick={e => { e.stopPropagation(); onEdit() }}
          title="Edit"
          className="text-text-muted hover:text-text-primary px-0.5 leading-none"
        >
          ✎
        </button>
        <button
          onClick={e => { e.stopPropagation(); onSync() }}
          disabled={isSyncing}
          title="Sync"
          className="text-text-muted hover:text-text-primary px-0.5 leading-none disabled:opacity-40"
        >
          ↻
        </button>
        <button
          onClick={e => { e.stopPropagation(); onDelete() }}
          title="Delete"
          className="text-text-muted hover:text-destructive px-0.5 leading-none"
        >
          ×
        </button>
      </span>
    </div>
  )
}
