import type { Library } from '../api/libraries'

interface SourceItem {
  library_id: number
  priority: number
  library_name?: string
}

interface Props {
  sources: SourceItem[]
  allLibraries: Library[]
  onChange: (updated: Array<{ library_id: number; priority: number }>) => void
}

function renumber(items: Array<{ library_id: number }>): Array<{ library_id: number; priority: number }> {
  return items.map((item, i) => ({ library_id: item.library_id, priority: i + 1 }))
}

export function SourcePriorityList({ sources, allLibraries, onChange }: Props) {
  const usedIds = new Set(sources.map(s => s.library_id))
  const available = allLibraries.filter(lib => !usedIds.has(lib.id))

  function moveUp(index: number) {
    if (index === 0) return
    const next = [...sources]
    ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
    onChange(renumber(next))
  }

  function moveDown(index: number) {
    if (index === sources.length - 1) return
    const next = [...sources]
    ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
    onChange(renumber(next))
  }

  function remove(index: number) {
    const next = sources.filter((_, i) => i !== index)
    onChange(renumber(next))
  }

  function add(libraryId: number) {
    const next = [...sources, { library_id: libraryId, priority: sources.length + 1 }]
    onChange(renumber(next))
  }

  function getLibraryName(libraryId: number): string {
    const found = allLibraries.find(l => l.id === libraryId)
    return found?.name ?? `Library #${libraryId}`
  }

  return (
    <div className="flex flex-col gap-1">
      {sources.length === 0 ? (
        <p className="text-text-muted text-xs italic">No source libraries — add one below.</p>
      ) : (
        <div className="border border-border rounded overflow-hidden">
          {sources.map((src, i) => (
            <div
              key={src.library_id}
              className="flex items-center gap-2 px-2 py-1.5 border-b border-border-subtle last:border-b-0 hover:bg-bg-row-hover"
            >
              <span className="text-text-muted text-[9px] w-4 text-right shrink-0">{src.priority}</span>
              <span className="text-text-primary text-xs flex-1 truncate">
                {src.library_name ?? getLibraryName(src.library_id)}
              </span>
              <div className="flex items-center gap-1 shrink-0">
                <button
                  type="button"
                  onClick={() => moveUp(i)}
                  disabled={i === 0}
                  className="text-text-muted hover:text-text-primary disabled:opacity-30 text-xs px-0.5"
                  title="Move up"
                >
                  ▲
                </button>
                <button
                  type="button"
                  onClick={() => moveDown(i)}
                  disabled={i === sources.length - 1}
                  className="text-text-muted hover:text-text-primary disabled:opacity-30 text-xs px-0.5"
                  title="Move down"
                >
                  ▼
                </button>
                <button
                  type="button"
                  onClick={() => remove(i)}
                  className="text-text-muted hover:text-destructive text-xs px-0.5"
                  title="Remove"
                >
                  ✕
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {available.length > 0 && (
        <select
          value=""
          onChange={e => {
            const id = Number(e.target.value)
            if (id) add(id)
          }}
          className="mt-1 bg-bg-base border border-border text-text-muted text-xs px-2 py-1 rounded focus:outline-none focus:border-accent"
        >
          <option value="" disabled>+ Add source library…</option>
          {available.map(lib => (
            <option key={lib.id} value={lib.id}>{lib.name}</option>
          ))}
        </select>
      )}
    </div>
  )
}
