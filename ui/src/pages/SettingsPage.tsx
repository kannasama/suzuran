import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { EncodingProfileForm } from '../components/EncodingProfileForm'
import { ArtProfileForm } from '../components/ArtProfileForm'
import { VirtualLibraryForm } from '../components/VirtualLibraryForm'
import { ImageUpload } from '../components/ImageUpload'
import {
  listEncodingProfiles,
  createEncodingProfile,
  updateEncodingProfile,
  deleteEncodingProfile,
} from '../api/encodingProfiles'
import {
  listArtProfiles,
  createArtProfile,
  updateArtProfile,
  deleteArtProfile,
} from '../api/artProfiles'
import {
  listVirtualLibraries,
  createVirtualLibrary,
  updateVirtualLibrary,
  deleteVirtualLibrary,
  getSources,
  setSources,
  triggerSync,
} from '../api/virtualLibraries'
import {
  listThemes,
  createTheme,
  updateTheme,
  deleteTheme,
  type Theme,
  type UpsertTheme,
} from '../api/themes'
import type { EncodingProfile, UpsertEncodingProfile } from '../types/encodingProfile'
import type { ArtProfile, UpsertArtProfile } from '../types/artProfile'
import type { VirtualLibrary, UpsertVirtualLibrary } from '../types/virtualLibrary'

type ActiveTab = 'encoding' | 'art' | 'virtual' | 'themes'

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<ActiveTab>('encoding')

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <main className="flex-1 overflow-y-auto">
        {/* Tab bar */}
        <div className="flex items-center gap-0 border-b border-border px-6 bg-bg-surface flex-shrink-0">
          <TabButton label="Encoding Profiles" active={activeTab === 'encoding'} onClick={() => setActiveTab('encoding')} />
          <TabButton label="Art Profiles" active={activeTab === 'art'} onClick={() => setActiveTab('art')} />
          <TabButton label="Virtual Libraries" active={activeTab === 'virtual'} onClick={() => setActiveTab('virtual')} />
          <TabButton label="Themes" active={activeTab === 'themes'} onClick={() => setActiveTab('themes')} />
        </div>

        <div className="p-6">
          {activeTab === 'encoding' && <EncodingProfilesSection />}
          {activeTab === 'art' && <ArtProfilesSection />}
          {activeTab === 'virtual' && <VirtualLibrariesSection />}
          {activeTab === 'themes' && <ThemesSection />}
        </div>
      </main>
    </div>
  )
}

function TabButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`text-xs px-4 py-2.5 border-b-2 transition-colors ${
        active
          ? 'text-accent border-accent'
          : 'text-text-muted border-transparent hover:text-text-secondary'
      }`}
    >
      {label}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Encoding Profiles section
// ---------------------------------------------------------------------------

function EncodingProfilesSection() {
  const qc = useQueryClient()
  const { data: profiles = [], isLoading } = useQuery({
    queryKey: ['encoding-profiles'],
    queryFn: listEncodingProfiles,
  })
  const [editing, setEditing] = useState<EncodingProfile | 'new' | null>(null)

  const createMutation = useMutation({
    mutationFn: (data: UpsertEncodingProfile) => createEncodingProfile(data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['encoding-profiles'] }); setEditing(null) },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpsertEncodingProfile }) => updateEncodingProfile(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['encoding-profiles'] }); setEditing(null) },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteEncodingProfile(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['encoding-profiles'] }),
  })

  const isSavePending = createMutation.isPending || updateMutation.isPending

  async function handleSave(data: UpsertEncodingProfile) {
    if (editing === 'new') {
      await createMutation.mutateAsync(data)
    } else if (editing != null) {
      await updateMutation.mutateAsync({ id: editing.id, data })
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-text-primary font-semibold text-sm">Encoding Profiles</h1>
        {editing == null && (
          <button
            onClick={() => setEditing('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Profile
          </button>
        )}
      </div>

      {editing != null && (
        <div className="mb-5 bg-bg-panel border border-border rounded p-4 max-w-lg">
          <p className="text-text-muted text-xs uppercase tracking-wider mb-3">
            {editing === 'new' ? 'New Encoding Profile' : `Edit: ${editing.name}`}
          </p>
          <EncodingProfileForm
            initial={editing === 'new' ? undefined : {
              name: editing.name,
              codec: editing.codec,
              bitrate: editing.bitrate,
              sample_rate: editing.sample_rate,
              channels: editing.channels,
              bit_depth: editing.bit_depth,
              advanced_args: editing.advanced_args,
            }}
            onSave={handleSave}
            onCancel={() => setEditing(null)}
            isPending={isSavePending}
          />
        </div>
      )}

      {isLoading ? (
        <p className="text-text-muted text-xs">Loading…</p>
      ) : profiles.length === 0 && editing == null ? (
        <div className="flex flex-col items-center justify-center py-16 gap-2">
          <p className="text-text-muted text-xs">No encoding profiles defined.</p>
          <p className="text-text-muted text-xs">Profiles configure the output codec and quality for transcoding.</p>
        </div>
      ) : profiles.length > 0 ? (
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-border text-text-muted text-[9px] uppercase tracking-wider">
              <th className="text-left pb-2 pr-4 font-medium">Name</th>
              <th className="text-left pb-2 pr-4 font-medium">Codec</th>
              <th className="text-left pb-2 pr-4 font-medium">Bitrate</th>
              <th className="text-left pb-2 pr-4 font-medium">Sample Rate</th>
              <th className="text-left pb-2 pr-4 font-medium">Channels</th>
              <th className="pb-2"></th>
            </tr>
          </thead>
          <tbody>
            {profiles.map(p => (
              <tr key={p.id} className="border-b border-border-subtle hover:bg-bg-panel">
                <td className="py-1.5 pr-4 text-text-primary font-medium">{p.name}</td>
                <td className="py-1.5 pr-4 text-text-muted font-mono">{p.codec}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.bitrate ?? '—'}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.sample_rate ?? '—'}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.channels ?? '—'}</td>
                <td className="py-1.5 pl-2">
                  <div className="flex gap-2 justify-end">
                    <button
                      onClick={() => setEditing(p)}
                      className="text-text-muted hover:text-text-primary"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => {
                        if (window.confirm(`Delete encoding profile "${p.name}"?`)) {
                          deleteMutation.mutate(p.id)
                        }
                      }}
                      className="text-text-muted hover:text-destructive"
                    >
                      Delete
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Art Profiles section
// ---------------------------------------------------------------------------

function ArtProfilesSection() {
  const qc = useQueryClient()
  const { data: profiles = [], isLoading } = useQuery({
    queryKey: ['art-profiles'],
    queryFn: listArtProfiles,
  })
  const [editing, setEditing] = useState<ArtProfile | 'new' | null>(null)

  const createMutation = useMutation({
    mutationFn: (data: UpsertArtProfile) => createArtProfile(data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['art-profiles'] }); setEditing(null) },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpsertArtProfile }) => updateArtProfile(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['art-profiles'] }); setEditing(null) },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteArtProfile(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['art-profiles'] }),
  })

  const isSavePending = createMutation.isPending || updateMutation.isPending

  async function handleSave(data: UpsertArtProfile) {
    if (editing === 'new') {
      await createMutation.mutateAsync(data)
    } else if (editing != null) {
      await updateMutation.mutateAsync({ id: editing.id, data })
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-text-primary font-semibold text-sm">Art Profiles</h1>
        {editing == null && (
          <button
            onClick={() => setEditing('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Profile
          </button>
        )}
      </div>

      {editing != null && (
        <div className="mb-5 bg-bg-panel border border-border rounded p-4 max-w-lg">
          <p className="text-text-muted text-xs uppercase tracking-wider mb-3">
            {editing === 'new' ? 'New Art Profile' : `Edit: ${editing.name}`}
          </p>
          <ArtProfileForm
            initial={editing === 'new' ? undefined : {
              name: editing.name,
              format: editing.format,
              quality: editing.quality,
              max_width_px: editing.max_width_px,
              max_height_px: editing.max_height_px,
              max_size_bytes: editing.max_size_bytes,
              apply_to_library_id: editing.apply_to_library_id,
            }}
            onSave={handleSave}
            onCancel={() => setEditing(null)}
            isPending={isSavePending}
          />
        </div>
      )}

      {isLoading ? (
        <p className="text-text-muted text-xs">Loading…</p>
      ) : profiles.length === 0 && editing == null ? (
        <div className="flex flex-col items-center justify-center py-16 gap-2">
          <p className="text-text-muted text-xs">No art profiles defined.</p>
          <p className="text-text-muted text-xs">Profiles configure cover art resizing and recompression.</p>
        </div>
      ) : profiles.length > 0 ? (
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-border text-text-muted text-[9px] uppercase tracking-wider">
              <th className="text-left pb-2 pr-4 font-medium">Name</th>
              <th className="text-left pb-2 pr-4 font-medium">Format</th>
              <th className="text-left pb-2 pr-4 font-medium">Quality</th>
              <th className="text-left pb-2 pr-4 font-medium">Max Size</th>
              <th className="text-left pb-2 pr-4 font-medium">Max Bytes</th>
              <th className="pb-2"></th>
            </tr>
          </thead>
          <tbody>
            {profiles.map(p => (
              <tr key={p.id} className="border-b border-border-subtle hover:bg-bg-panel">
                <td className="py-1.5 pr-4 text-text-primary font-medium">{p.name}</td>
                <td className="py-1.5 pr-4 text-text-muted font-mono">{p.format}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.quality}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.max_width_px}×{p.max_height_px}</td>
                <td className="py-1.5 pr-4 text-text-muted">{p.max_size_bytes != null ? p.max_size_bytes.toLocaleString() : '—'}</td>
                <td className="py-1.5 pl-2">
                  <div className="flex gap-2 justify-end">
                    <button
                      onClick={() => setEditing(p)}
                      className="text-text-muted hover:text-text-primary"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => {
                        if (window.confirm(`Delete art profile "${p.name}"?`)) {
                          deleteMutation.mutate(p.id)
                        }
                      }}
                      className="text-text-muted hover:text-destructive"
                    >
                      Delete
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Virtual Libraries section
// ---------------------------------------------------------------------------

function VirtualLibrariesSection() {
  const qc = useQueryClient()
  const { data: vlibs = [], isLoading } = useQuery({
    queryKey: ['virtual-libraries'],
    queryFn: listVirtualLibraries,
  })
  const [editing, setEditing] = useState<VirtualLibrary | 'new' | null>(null)
  const [syncingId, setSyncingId] = useState<number | null>(null)

  const createMutation = useMutation({
    mutationFn: (data: UpsertVirtualLibrary) => createVirtualLibrary(data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['virtual-libraries'] }); setEditing(null) },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpsertVirtualLibrary }) => updateVirtualLibrary(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['virtual-libraries'] }); setEditing(null) },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteVirtualLibrary(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['virtual-libraries'] }),
  })

  const isSavePending = createMutation.isPending || updateMutation.isPending

  async function handleSave(
    data: UpsertVirtualLibrary,
    sources: Array<{ library_id: number; priority: number }>,
  ) {
    if (editing === 'new') {
      const created = await createMutation.mutateAsync(data)
      await setSources(created.id, sources)
      qc.invalidateQueries({ queryKey: ['virtual-libraries'] })
    } else if (editing != null) {
      await updateMutation.mutateAsync({ id: editing.id, data })
      await setSources(editing.id, sources)
      qc.invalidateQueries({ queryKey: ['virtual-libraries'] })
    }
  }

  async function handleSync(id: number) {
    setSyncingId(id)
    try {
      await triggerSync(id)
    } finally {
      setSyncingId(null)
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-text-primary font-semibold text-sm">Virtual Libraries</h1>
        {editing == null && (
          <button
            onClick={() => setEditing('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Virtual Library
          </button>
        )}
      </div>

      {editing != null && (
        <div className="mb-5 bg-bg-panel border border-border rounded p-4 max-w-lg">
          <p className="text-text-muted text-xs uppercase tracking-wider mb-3">
            {editing === 'new' ? 'New Virtual Library' : `Edit: ${editing.name}`}
          </p>
          <VirtualLibraryForm
            initial={editing === 'new' ? undefined : editing}
            onSave={handleSave}
            onCancel={() => setEditing(null)}
            isPending={isSavePending}
          />
        </div>
      )}

      {isLoading ? (
        <p className="text-text-muted text-xs">Loading…</p>
      ) : vlibs.length === 0 && editing == null ? (
        <div className="flex flex-col items-center justify-center py-16 gap-2">
          <p className="text-text-muted text-xs">No virtual libraries defined.</p>
          <p className="text-text-muted text-xs">Virtual libraries aggregate tracks from multiple source libraries via symlinks or hardlinks.</p>
        </div>
      ) : vlibs.length > 0 ? (
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-border text-text-muted text-[9px] uppercase tracking-wider">
              <th className="text-left pb-2 pr-4 font-medium">Name</th>
              <th className="text-left pb-2 pr-4 font-medium">Link Type</th>
              <th className="text-left pb-2 pr-4 font-medium">Root Path</th>
              <th className="text-left pb-2 pr-4 font-medium">Sources</th>
              <th className="pb-2"></th>
            </tr>
          </thead>
          <tbody>
            {vlibs.map(v => (
              <VirtualLibraryRow
                key={v.id}
                vlib={v}
                isSyncing={syncingId === v.id}
                onEdit={() => setEditing(v)}
                onDelete={() => {
                  if (window.confirm(`Delete virtual library "${v.name}"?`)) {
                    deleteMutation.mutate(v.id)
                  }
                }}
                onSync={() => handleSync(v.id)}
              />
            ))}
          </tbody>
        </table>
      ) : null}
    </div>
  )
}

function VirtualLibraryRow({
  vlib,
  isSyncing,
  onEdit,
  onDelete,
  onSync,
}: {
  vlib: VirtualLibrary
  isSyncing: boolean
  onEdit: () => void
  onDelete: () => void
  onSync: () => void
}) {
  const { data: sources = [] } = useQuery({
    queryKey: ['virtual-library-sources', vlib.id],
    queryFn: () => getSources(vlib.id),
  })

  return (
    <tr className="border-b border-border-subtle hover:bg-bg-panel">
      <td className="py-1.5 pr-4 text-text-primary font-medium">{vlib.name}</td>
      <td className="py-1.5 pr-4 text-text-muted font-mono">{vlib.link_type}</td>
      <td className="py-1.5 pr-4 text-text-muted font-mono truncate max-w-[200px]">{vlib.root_path}</td>
      <td className="py-1.5 pr-4 text-text-muted">{sources.length}</td>
      <td className="py-1.5 pl-2">
        <div className="flex gap-2 justify-end">
          <button
            onClick={onEdit}
            className="text-text-muted hover:text-text-primary"
          >
            Edit
          </button>
          <button
            onClick={onSync}
            disabled={isSyncing}
            className="text-text-muted hover:text-text-secondary disabled:opacity-40"
          >
            {isSyncing ? 'Syncing…' : 'Sync'}
          </button>
          <button
            onClick={onDelete}
            className="text-text-muted hover:text-destructive"
          >
            Delete
          </button>
        </div>
      </td>
    </tr>
  )
}

// ---------------------------------------------------------------------------
// Themes section
// ---------------------------------------------------------------------------

function ThemesSection() {
  const qc = useQueryClient()
  const { data: themes = [], isLoading } = useQuery({
    queryKey: ['themes'],
    queryFn: listThemes,
  })
  const [editing, setEditing] = useState<Theme | 'new' | null>(null)
  const [editingTheme, setEditingTheme] = useState<UpsertTheme>({
    name: '',
    css_vars: {},
    accent_color: null,
    background_url: null,
  })

  const createMutation = useMutation({
    mutationFn: (data: UpsertTheme) => createTheme(data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['themes'] }); setEditing(null) },
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: number; data: UpsertTheme }) => updateTheme(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['themes'] }); setEditing(null) },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteTheme(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['themes'] }),
  })

  const isSavePending = createMutation.isPending || updateMutation.isPending

  function startEdit(theme: Theme | 'new') {
    setEditing(theme)
    if (theme === 'new') {
      setEditingTheme({ name: '', css_vars: {}, accent_color: null, background_url: null })
    } else {
      setEditingTheme({
        name: theme.name,
        css_vars: theme.css_vars,
        accent_color: theme.accent_color,
        background_url: theme.background_url,
      })
    }
  }

  async function handleSave() {
    if (editing === 'new') {
      await createMutation.mutateAsync(editingTheme)
    } else if (editing != null) {
      await updateMutation.mutateAsync({ id: editing.id, data: editingTheme })
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-text-primary font-semibold text-sm">Themes</h1>
        {editing == null && (
          <button
            onClick={() => startEdit('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Theme
          </button>
        )}
      </div>

      {editing != null && (
        <div className="mb-5 bg-bg-panel border border-border rounded p-4 max-w-lg">
          <p className="text-text-muted text-xs uppercase tracking-wider mb-3">
            {editing === 'new' ? 'New Theme' : `Edit: ${editing.name}`}
          </p>
          <div className="space-y-3">
            <div>
              <label className="block text-xs text-text-muted mb-1">Name</label>
              <input
                type="text"
                value={editingTheme.name}
                onChange={e => setEditingTheme(t => ({ ...t, name: e.target.value }))}
                className="w-full text-sm bg-bg-panel text-text-primary border border-border rounded px-2 py-1"
                placeholder="My Theme"
              />
            </div>
            <div>
              <label className="block text-xs text-text-muted mb-1">Accent color</label>
              <input
                type="text"
                value={editingTheme.accent_color ?? ''}
                onChange={e => setEditingTheme(t => ({ ...t, accent_color: e.target.value || null }))}
                className="w-full text-sm bg-bg-panel text-text-primary border border-border rounded px-2 py-1"
                placeholder="#6366f1"
              />
            </div>
            <ImageUpload
              value={editingTheme.background_url ?? ''}
              onChange={url => setEditingTheme(t => ({ ...t, background_url: url || null }))}
            />
          </div>
          <div className="flex gap-2 mt-4">
            <button
              onClick={handleSave}
              disabled={isSavePending}
              className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
            >
              {isSavePending ? 'Saving…' : 'Save'}
            </button>
            <button
              onClick={() => setEditing(null)}
              className="text-xs text-text-muted hover:text-text-primary px-3 py-1"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {isLoading ? (
        <p className="text-text-muted text-xs">Loading…</p>
      ) : themes.length === 0 && editing == null ? (
        <div className="flex flex-col items-center justify-center py-16 gap-2">
          <p className="text-text-muted text-xs">No custom themes defined.</p>
          <p className="text-text-muted text-xs">Themes can set an accent color and a background image.</p>
        </div>
      ) : themes.length > 0 ? (
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-border text-text-muted text-[9px] uppercase tracking-wider">
              <th className="text-left pb-2 pr-4 font-medium">Name</th>
              <th className="text-left pb-2 pr-4 font-medium">Accent</th>
              <th className="text-left pb-2 pr-4 font-medium">Background</th>
              <th className="pb-2"></th>
            </tr>
          </thead>
          <tbody>
            {themes.map(t => (
              <tr key={t.id} className="border-b border-border-subtle hover:bg-bg-panel">
                <td className="py-1.5 pr-4 text-text-primary font-medium">{t.name}</td>
                <td className="py-1.5 pr-4 text-text-muted font-mono">{t.accent_color ?? '—'}</td>
                <td className="py-1.5 pr-4 text-text-muted truncate max-w-[200px]">
                  {t.background_url ? (
                    <span className="font-mono">{t.background_url}</span>
                  ) : '—'}
                </td>
                <td className="py-1.5 pl-2">
                  <div className="flex gap-2 justify-end">
                    <button
                      onClick={() => startEdit(t)}
                      className="text-text-muted hover:text-text-primary"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => {
                        if (window.confirm(`Delete theme "${t.name}"?`)) {
                          deleteMutation.mutate(t.id)
                        }
                      }}
                      className="text-text-muted hover:text-destructive"
                    >
                      Delete
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </div>
  )
}
