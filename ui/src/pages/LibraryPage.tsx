import { useState, useEffect, useRef } from 'react'
import { TopNav } from '../components/TopNav'
import { LibraryTree } from '../components/LibraryTree'
import { TranscodeDialog } from '../components/TranscodeDialog'
import { useAuth } from '../contexts/AuthContext'

interface ColumnDef {
  key: string
  label: string        // shown in the picker
  headerLabel?: string // shown in the column header (falls back to label)
  className: string
}

const COLUMNS: ColumnDef[] = [
  { key: 'num',      label: 'Track #',  headerLabel: '#',    className: 'w-6' },
  { key: 'title',    label: 'Title',                         className: 'flex-[3]' },
  { key: 'artist',   label: 'Artist',                        className: 'flex-[2]' },
  { key: 'album',    label: 'Album',                         className: 'flex-[2]' },
  { key: 'year',     label: 'Year',                          className: 'w-10' },
  { key: 'genre',    label: 'Genre',                         className: 'flex-1' },
  { key: 'format',   label: 'Format',                        className: 'w-12' },
  { key: 'bitrate',  label: 'Bitrate',                       className: 'w-14' },
  { key: 'duration', label: 'Duration', headerLabel: 'Time', className: 'w-10' },
  { key: 'actions',  label: 'Actions',                       className: 'w-16' },
]

const LS_KEY = 'suzuran:column-visibility'

function loadColumnVisibility(): Set<string> {
  try {
    const raw = localStorage.getItem(LS_KEY)
    if (raw) {
      const arr = JSON.parse(raw)
      if (Array.isArray(arr)) return new Set(arr as string[])
    }
  } catch { /* ignore */ }
  return new Set(COLUMNS.map(c => c.key))
}

export function LibraryPage() {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'

  const [selectedLibraryId, setSelectedLibraryId] = useState<number | null>(null)
  const [selectedVirtualLibraryId, setSelectedVirtualLibraryId] = useState<number | null>(null)
  const [transcodeDialog, setTranscodeDialog] = useState<
    { mode: 'track' | 'library'; sourceId: number } | null
  >(null)
  const [visibleColumns, setVisibleColumns] = useState<Set<string>>(loadColumnVisibility)
  const [showColumnPicker, setShowColumnPicker] = useState(false)
  const pickerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!showColumnPicker) return
    function handleMouseDown(e: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowColumnPicker(false)
      }
    }
    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [showColumnPicker])

  function toggleColumn(key: string) {
    setVisibleColumns(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      localStorage.setItem(LS_KEY, JSON.stringify([...next]))
      return next
    })
  }

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-1 overflow-hidden">
        {/* Left: tree pane */}
        <aside className="w-44 flex-shrink-0 bg-bg-panel border-r border-border overflow-y-auto">
          <LibraryTree
            isAdmin={isAdmin}
            selectedLibraryId={selectedLibraryId}
            onSelectLibrary={id => { setSelectedLibraryId(id); setSelectedVirtualLibraryId(null) }}
            selectedVirtualLibraryId={selectedVirtualLibraryId}
            onSelectVirtualLibrary={id => { setSelectedVirtualLibraryId(id); setSelectedLibraryId(null) }}
          />
        </aside>

        {/* Right: track list */}
        <main className="flex flex-col flex-1 overflow-hidden">
          {/* Toolbar */}
          <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
            <span className="text-text-muted text-xs">
              {selectedLibraryId == null && selectedVirtualLibraryId == null
                ? 'Select a library'
                : selectedVirtualLibraryId != null
                  ? `Virtual Library #${selectedVirtualLibraryId}`
                  : `Library #${selectedLibraryId}`}
            </span>
            <div className="ml-auto flex gap-1">
              {selectedLibraryId != null && selectedVirtualLibraryId == null && (
                <button
                  onClick={() => setTranscodeDialog({ mode: 'library', sourceId: selectedLibraryId })}
                  className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:text-text-primary hover:border-accent"
                  title="Transcode this library to a derived library"
                >
                  Transcode ▾
                </button>
              )}
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Group: None ▾
              </button>
              <button className="text-xs text-text-muted bg-bg-panel border border-border rounded px-2 py-0.5 hover:border-border">
                Sort ▾
              </button>
            </div>
          </div>

          {/* Column headers */}
          <div className="flex items-center gap-0 px-2 py-1 bg-bg-panel border-b border-border text-text-muted text-[11px] uppercase tracking-wider flex-shrink-0">
            <span className="w-5 shrink-0"></span>
            {COLUMNS.map(col => visibleColumns.has(col.key) && (
              <span key={col.key} className={col.className + ' shrink-0'}>
                {col.headerLabel ?? col.label}
              </span>
            ))}
            {/* Column picker */}
            <div ref={pickerRef} className="relative w-6 shrink-0 ml-auto">
              <span
                className="text-accent cursor-pointer hover:opacity-70 transition-opacity block text-center"
                onClick={() => setShowColumnPicker(v => !v)}
                title="Customize columns"
              >
                ⊕
              </span>
              {showColumnPicker && (
                <div className="absolute right-0 top-full mt-1 z-50 bg-bg-panel border border-border rounded shadow-lg py-1 min-w-[140px]">
                  {COLUMNS.map(col => (
                    <label
                      key={col.key}
                      className="flex items-center gap-2 px-3 py-1 hover:bg-bg-row-hover cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={visibleColumns.has(col.key)}
                        onChange={() => toggleColumn(col.key)}
                        className="accent-[color:var(--accent)]"
                      />
                      <span className="text-text-primary text-xs normal-case tracking-normal">
                        {col.label}
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Track list area (stub — populated in a future subphase) */}
          <div className="flex-1 overflow-y-auto">
            <div className="flex items-center justify-center h-32 text-text-muted text-xs">
              {selectedLibraryId == null && selectedVirtualLibraryId == null
                ? 'Select a library from the tree to view tracks.'
                : 'Track list coming in a future subphase.'}
            </div>
          </div>
        </main>
      </div>

      {/* Transcode dialog */}
      {transcodeDialog != null && (
        <TranscodeDialog
          mode={transcodeDialog.mode}
          sourceId={transcodeDialog.sourceId}
          onClose={() => setTranscodeDialog(null)}
        />
      )}
    </div>
  )
}
