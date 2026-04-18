import { useQuery } from '@tanstack/react-query'
import { listLibraries, type Library } from '../api/libraries'

export function LibraryTree() {
  const { data: libraries = [], isLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  if (isLoading) {
    return (
      <div className="p-3 text-text-muted text-xs">Loading…</div>
    )
  }

  return (
    <div className="flex flex-col overflow-y-auto text-xs">
      <div className="px-2 py-1 mb-1 border-b border-border-subtle">
        <input
          type="search"
          placeholder="Search…"
          className="w-full bg-bg-base border border-border text-text-primary text-xs px-2 py-1 rounded focus:outline-none focus:border-accent"
        />
      </div>
      {libraries.map(lib => (
        <LibraryNode key={lib.id} library={lib} />
      ))}
    </div>
  )
}

function LibraryNode({ library }: { library: Library }) {
  return (
    <div>
      <div className="px-2 py-0.5 text-accent bg-accent-muted border-l-2 border-accent flex justify-between items-center cursor-pointer">
        <span>▾ {library.name}</span>
        <span className="text-text-muted uppercase text-[9px] tracking-wider">
          {library.format}
        </span>
      </div>
      <div className="pl-4 py-0.5 text-text-secondary cursor-pointer hover:bg-bg-hover">
        ▾ Artists
      </div>
      <div className="pl-4 py-0.5 text-text-muted cursor-pointer hover:bg-bg-hover">
        ▸ Albums
      </div>
      <div className="pl-4 py-0.5 text-text-muted cursor-pointer hover:bg-bg-hover">
        ▸ Genres
      </div>
    </div>
  )
}
