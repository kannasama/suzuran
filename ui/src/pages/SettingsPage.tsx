import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { EncodingProfileForm } from '../components/EncodingProfileForm'
import { ArtProfileForm } from '../components/ArtProfileForm'
import { VirtualLibraryForm } from '../components/VirtualLibraryForm'
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
import { listSettings, setSetting } from '../api/settings'
import { useTheme } from '../theme/ThemeProvider'
import { extractPalette, hslToRgbStr } from '../utils/extractPalette'
import type { EncodingProfile, UpsertEncodingProfile } from '../types/encodingProfile'
import type { ArtProfile, UpsertArtProfile } from '../types/artProfile'
import type { VirtualLibrary, UpsertVirtualLibrary } from '../types/virtualLibrary'

type ActiveTab = 'general' | 'encoding' | 'art' | 'virtual' | 'themes'

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<ActiveTab>('general')

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <main className="flex-1 overflow-y-auto">
        {/* Tab bar */}
        <div className="flex items-center gap-0 border-b border-border px-6 bg-bg-surface flex-shrink-0">
          <TabButton label="General" active={activeTab === 'general'} onClick={() => setActiveTab('general')} />
          <TabButton label="Encoding Profiles" active={activeTab === 'encoding'} onClick={() => setActiveTab('encoding')} />
          <TabButton label="Art Profiles" active={activeTab === 'art'} onClick={() => setActiveTab('art')} />
          <TabButton label="Virtual Libraries" active={activeTab === 'virtual'} onClick={() => setActiveTab('virtual')} />
          <TabButton label="Themes" active={activeTab === 'themes'} onClick={() => setActiveTab('themes')} />
        </div>

        <div className="p-6">
          {activeTab === 'general' && <GeneralSettingsSection />}
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
// General Settings section
// ---------------------------------------------------------------------------

const SETTING_META: Record<string, { label: string; description: string; type: 'text' | 'number' | 'password' | 'boolean' }> = {
  acoustid_api_key:        { label: 'AcoustID API Key',          description: 'Required for acoustic fingerprint lookups. Get a free key at acoustid.org.',  type: 'password' },
  mb_user_agent:           { label: 'MusicBrainz User Agent',    description: 'Sent with every MusicBrainz request. Must identify your application.',        type: 'text'     },
  mb_rate_limit_ms:        { label: 'MusicBrainz Rate Limit (ms)', description: 'Minimum delay between MusicBrainz requests. Default: 1100.',                type: 'number'   },
  mb_confidence_threshold: { label: 'MB Confidence Threshold',   description: 'Minimum AcoustID score (0–1) to create a tag suggestion. Default: 0.8.',      type: 'number'   },
  scan_concurrency:        { label: 'Scan Concurrency',          description: 'Number of parallel file scan workers.',                                        type: 'number'   },
  transcode_concurrency:   { label: 'Transcode Concurrency',     description: 'Number of parallel transcode jobs.',                                           type: 'number'   },
  default_art_profile_id:  { label: 'Default Art Profile ID',    description: 'Art profile applied when no library-specific profile is set.',                  type: 'number'   },
  allow_registration:      { label: 'Allow Registration',        description: 'Show the Register link on the login page. Disable after initial setup.',       type: 'boolean'  },
}

const SETTING_ORDER = [
  'acoustid_api_key',
  'mb_user_agent',
  'mb_rate_limit_ms',
  'mb_confidence_threshold',
  'scan_concurrency',
  'transcode_concurrency',
  'default_art_profile_id',
  'allow_registration',
]

function GeneralSettingsSection() {
  const qc = useQueryClient()
  const { data: settings = [], isLoading } = useQuery({
    queryKey: ['settings'],
    queryFn: listSettings,
  })

  const settingMap = Object.fromEntries(settings.map(s => [s.key, s.value]))

  const [drafts, setDrafts] = useState<Record<string, string>>({})
  const [saving, setSaving] = useState<Record<string, boolean>>({})
  const [saved, setSaved] = useState<Record<string, boolean>>({})
  const [errors, setErrors] = useState<Record<string, string>>({})

  function getValue(key: string) {
    return key in drafts ? drafts[key] : (settingMap[key] ?? '')
  }

  function handleChange(key: string, value: string) {
    setDrafts(d => ({ ...d, [key]: value }))
    setSaved(s => ({ ...s, [key]: false }))
  }

  async function handleSave(key: string) {
    setSaving(s => ({ ...s, [key]: true }))
    setErrors(e => ({ ...e, [key]: '' }))
    try {
      await setSetting(key, getValue(key))
      qc.invalidateQueries({ queryKey: ['settings'] })
      setDrafts(d => { const n = { ...d }; delete n[key]; return n })
      setSaved(s => ({ ...s, [key]: true }))
      setTimeout(() => setSaved(s => ({ ...s, [key]: false })), 2000)
    } catch (err) {
      setErrors(e => ({ ...e, [key]: err instanceof Error ? err.message : 'Save failed' }))
    } finally {
      setSaving(s => ({ ...s, [key]: false }))
    }
  }

  if (isLoading) return <p className="text-text-muted text-xs">Loading…</p>

  return (
    <div className="max-w-lg">
      <h1 className="text-text-primary font-semibold text-sm mb-5">General Settings</h1>
      <div className="flex flex-col gap-5">
        {SETTING_ORDER.map(key => {
          const meta = SETTING_META[key]
          if (!meta) return null
          const isDirty = key in drafts

          if (meta.type === 'boolean') {
            const isEnabled = getValue(key) !== 'false'
            return (
              <div key={key} className="flex flex-col gap-1">
                <span className="text-text-muted text-xs uppercase tracking-wider">{meta.label}</span>
                <div className="flex gap-2 items-center">
                  <button
                    onClick={async () => {
                      const next = isEnabled ? 'false' : 'true'
                      setSaving(s => ({ ...s, [key]: true }))
                      setErrors(e => ({ ...e, [key]: '' }))
                      try {
                        await setSetting(key, next)
                        qc.invalidateQueries({ queryKey: ['settings'] })
                        qc.invalidateQueries({ queryKey: ['setup-status'] })
                      } catch (err) {
                        setErrors(e => ({ ...e, [key]: err instanceof Error ? err.message : 'Save failed' }))
                      } finally {
                        setSaving(s => ({ ...s, [key]: false }))
                      }
                    }}
                    disabled={saving[key]}
                    className={`text-xs rounded px-3 py-1.5 font-medium border transition-colors disabled:opacity-40 ${
                      isEnabled
                        ? 'bg-accent text-bg-base border-transparent hover:opacity-90'
                        : 'bg-transparent text-text-secondary border-border hover:border-accent hover:text-text-primary'
                    }`}
                  >
                    {saving[key] ? '…' : isEnabled ? 'Enabled' : 'Disabled'}
                  </button>
                </div>
                <p className="text-text-muted text-xs">{meta.description}</p>
                {errors[key] && <p className="text-destructive text-xs">{errors[key]}</p>}
              </div>
            )
          }

          return (
            <div key={key} className="flex flex-col gap-1">
              <label className="text-text-muted text-xs uppercase tracking-wider">{meta.label}</label>
              <div className="flex gap-2 items-center">
                <input
                  type={meta.type === 'password' ? 'password' : 'text'}
                  value={getValue(key)}
                  onChange={e => handleChange(key, e.target.value)}
                  autoComplete={meta.type === 'password' ? 'off' : undefined}
                  className="flex-1 bg-bg-panel text-text-primary border border-border text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
                />
                <button
                  onClick={() => handleSave(key)}
                  disabled={!isDirty || saving[key]}
                  className="text-xs text-bg-base bg-accent rounded px-3 py-1.5 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90 shrink-0"
                >
                  {saving[key] ? '…' : saved[key] ? 'Saved' : 'Save'}
                </button>
              </div>
              <p className="text-text-muted text-xs">{meta.description}</p>
              {errors[key] && <p className="text-destructive text-xs">{errors[key]}</p>}
            </div>
          )
        })}
      </div>
    </div>
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
              <tr key={p.id} className="border-b border-border-subtle hover:bg-bg-row-hover">
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
              <tr key={p.id} className="border-b border-border-subtle hover:bg-bg-row-hover">
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
    <tr className="border-b border-border-subtle hover:bg-bg-row-hover">
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
// Themes section — helpers
// ---------------------------------------------------------------------------

interface TextBrightness {
  secondary: number
  muted: number
  disabled: number
}

/**
 * Default text brightness for a given overlay darkness (0=darkest, 100=lightest).
 * Interpolates between standard dark and light text endpoint values.
 */
function computeDefaultTextBrightness(darkness: number): TextBrightness {
  const t = darkness / 100
  return {
    secondary: Math.round(82 + t * (27 - 82)),
    muted: 47,
    disabled: Math.round(35 + t * (66 - 35)),
  }
}

function hexToHsl(hex: string): [number, number, number] {
  const r = parseInt(hex.slice(1, 3), 16) / 255
  const g = parseInt(hex.slice(3, 5), 16) / 255
  const b = parseInt(hex.slice(5, 7), 16) / 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h = 0
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) * 60
  else if (max === g) h = ((b - r) / d + 2) * 60
  else h = ((r - g) / d + 4) * 60
  return [h, s, l]
}

/**
 * Compute surface CSS vars from an overlay darkness slider (0=dark, 100=light)
 * and an accent hex (for hue tinting). Opacity scales with distance from midpoint.
 */
function computeOverlayVars(accent: string, darkness: number, tb?: TextBrightness): Record<string, string> {
  const isDark = darkness < 50
  const { secondary, muted, disabled } = tb ?? computeDefaultTextBrightness(darkness)

  const bToHex = (b: number) => {
    const v = Math.round((b / 100) * 255)
    return '#' + [v, v, v].map(n => n.toString(16).padStart(2, '0')).join('')
  }

  const t = Math.abs(darkness - 50) / 50
  const opacity = 0.65 + t * 0.27
  const [h] = hexToHsl(accent)

  if (isDark) {
    return {
      '--bg-base':        `rgba(${hslToRgbStr(h, 0.15, 0.06)}, ${opacity.toFixed(2)})`,
      '--bg-surface':     `rgba(${hslToRgbStr(h, 0.12, 0.08)}, ${(opacity - 0.05).toFixed(2)})`,
      '--bg-panel':       `rgba(${hslToRgbStr(h, 0.18, 0.04)}, ${Math.min(opacity + 0.07, 0.97).toFixed(2)})`,
      '--bg-elevated':    `rgba(${hslToRgbStr(h, 0.10, 0.10)}, ${(opacity - 0.10).toFixed(2)})`,
      '--bg-row-hover':   'rgba(255, 255, 255, 0.06)',
      '--border':         `rgba(${hslToRgbStr(h, 0.10, 0.12)}, ${(opacity - 0.05).toFixed(2)})`,
      '--surface-border': `rgba(${hslToRgbStr(h, 0.10, 0.12)}, ${(opacity - 0.05).toFixed(2)})`,
      '--text-primary':   '#e8e8ec',
      '--text-secondary': bToHex(secondary),
      '--text-muted':     bToHex(muted),
      '--text-disabled':  bToHex(disabled),
    }
  } else {
    return {
      '--bg-base':        `rgba(${hslToRgbStr(h, 0.10, 0.97)}, ${opacity.toFixed(2)})`,
      '--bg-surface':     `rgba(${hslToRgbStr(h, 0.08, 0.99)}, ${(opacity - 0.04).toFixed(2)})`,
      '--bg-panel':       `rgba(${hslToRgbStr(h, 0.12, 0.94)}, ${Math.min(opacity + 0.06, 0.98).toFixed(2)})`,
      '--bg-elevated':    `rgba(${hslToRgbStr(h, 0.07, 0.96)}, ${(opacity - 0.08).toFixed(2)})`,
      '--bg-row-hover':   'rgba(0, 0, 0, 0.05)',
      '--border':         `rgba(${hslToRgbStr(h, 0.12, 0.88)}, ${(opacity - 0.04).toFixed(2)})`,
      '--surface-border': `rgba(${hslToRgbStr(h, 0.12, 0.88)}, ${(opacity - 0.04).toFixed(2)})`,
      '--text-primary':   '#0f0f18',
      '--text-secondary': bToHex(secondary),
      '--text-muted':     bToHex(muted),
      '--text-disabled':  bToHex(disabled),
    }
  }
}

function parseThemeVars(input: string): Record<string, string> {
  const trimmed = input.trim()
  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      return Object.fromEntries(
        Object.entries(parsed as Record<string, unknown>).map(([k, v]) => [k, String(v)])
      )
    }
    throw new Error('Expected a JSON object')
  } catch {
    // fall through to flat YAML
  }
  const result: Record<string, string> = {}
  for (const rawLine of trimmed.split('\n')) {
    const line = rawLine.trim()
    if (!line || line.startsWith('#')) continue
    const colonIdx = line.indexOf(':')
    if (colonIdx === -1) continue
    const key = line.slice(0, colonIdx).trim()
    if (!key.startsWith('--')) continue
    const val = line.slice(colonIdx + 1).trim().replace(/^["']|["']$/g, '')
    result[key] = val
  }
  if (Object.keys(result).length === 0) {
    throw new Error('No CSS custom properties found (expected --var-name: value lines)')
  }
  return result
}

// ---------------------------------------------------------------------------
// Themes section — edit panel
// ---------------------------------------------------------------------------

function ThemeEditPanel({
  title,
  initial,
  onSave,
  onCancel,
  isPending,
}: {
  title: string
  initial: UpsertTheme
  onSave: (data: UpsertTheme) => void
  onCancel: () => void
  isPending: boolean
}) {
  const [name, setName] = useState(initial.name)
  const [bgFile, setBgFile] = useState<File | null>(null)
  const [bgPreview, setBgPreview] = useState<string | null>(initial.background_url ?? null)
  const [extractedAccent, setExtractedAccent] = useState<string | null>(initial.accent_color ?? null)
  const [overlayDarkness, setOverlayDarkness] = useState(20)
  const [textBrightness, setTextBrightness] = useState<TextBrightness>(() => computeDefaultTextBrightness(20))
  const [varsText, setVarsText] = useState<string>(() => {
    const existing = initial.css_vars as Record<string, string>
    if (Object.keys(existing).length > 0) {
      return Object.entries(existing).map(([k, v]) => `${k}: "${v}"`).join('\n')
    }
    return ''
  })
  const [parseError, setParseError] = useState<string | null>(null)

  // Recompute overlay vars whenever slider or text brightness changes (only if palette extracted)
  useEffect(() => {
    const accent = extractedAccent
    if (!accent) return
    const vars = computeOverlayVars(accent, overlayDarkness, textBrightness)
    setVarsText(Object.entries(vars).map(([k, v]) => `${k}: "${v}"`).join('\n'))
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [overlayDarkness, extractedAccent, textBrightness])

  function doExtract(src: string) {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.onload = () => {
      const result = extractPalette(img)
      if (!result) { setParseError('Could not extract palette from image.'); return }
      setExtractedAccent(result.accent)
      setParseError(null)
      // Overlay effect will trigger via useEffect above
    }
    img.onerror = () => setParseError('Could not load image for extraction.')
    img.src = src
  }

  function handleBgSelect(f: File) {
    if (bgPreview && bgPreview !== initial.background_url) URL.revokeObjectURL(bgPreview)
    const url = URL.createObjectURL(f)
    setBgFile(f)
    setBgPreview(url)
    doExtract(url)
  }

  function handleClearBg() {
    if (bgPreview && bgPreview !== initial.background_url) URL.revokeObjectURL(bgPreview)
    setBgFile(null)
    setBgPreview(null)
    setExtractedAccent(null)
  }

  function handleSave() {
    let css_vars: Record<string, string> = {}
    if (varsText.trim()) {
      try {
        css_vars = parseThemeVars(varsText)
        setParseError(null)
      } catch (e) {
        setParseError(e instanceof Error ? e.message : 'Invalid CSS vars format.')
        return
      }
    }
    onSave({
      name,
      css_vars,
      accent_color: extractedAccent ?? null,
      background_url: bgFile ? null : (bgPreview ?? null), // bgFile will be uploaded separately
      _bgFile: bgFile ?? undefined,
    } as UpsertTheme & { _bgFile?: File })
  }

  return (
    <div className="mb-5 bg-bg-panel border border-border rounded p-4 max-w-lg">
      <div className="flex items-center justify-between mb-4">
        <p className="text-text-muted text-xs uppercase tracking-wider">{title}</p>
        <button onClick={onCancel} className="text-xs text-text-muted hover:text-text-primary transition-colors">
          Cancel
        </button>
      </div>

      <div className="flex flex-col gap-4">
        {/* Name */}
        <input
          type="text"
          value={name}
          onChange={e => setName(e.target.value)}
          placeholder="Theme name"
          autoFocus
          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
        />

        {/* Overlay darkness slider */}
        <div className="space-y-1">
          <div className="flex items-center justify-between text-[10px] text-text-disabled">
            <span>Dark overlay</span>
            <span>Overlay darkness</span>
            <span>Light overlay</span>
          </div>
          <input
            type="range"
            min={0}
            max={100}
            value={overlayDarkness}
            onChange={e => setOverlayDarkness(Number(e.target.value))}
            className="w-full accent-[var(--accent)]"
          />
        </div>

        {/* Advanced text tuning */}
        <details>
          <summary className="text-xs text-text-muted cursor-pointer select-none hover:text-text-secondary transition-colors">
            Advanced text tuning
          </summary>
          <div className="mt-3 space-y-3 pl-1">
            {(
              [
                { label: 'Secondary text', key: 'secondary' as const },
                { label: 'Muted text',     key: 'muted'     as const },
                { label: 'Disabled text',  key: 'disabled'  as const },
              ]
            ).map(({ label, key }) => {
              const def = computeDefaultTextBrightness(overlayDarkness)[key]
              return (
                <div key={key} className="flex items-center gap-2">
                  <span className="text-xs text-text-muted w-28 shrink-0">{label}</span>
                  <input
                    type="range"
                    min={0}
                    max={100}
                    value={textBrightness[key]}
                    onChange={e => setTextBrightness(prev => ({ ...prev, [key]: Number(e.target.value) }))}
                    className="flex-1 accent-[var(--accent)]"
                  />
                  <span className="text-xs text-text-disabled w-6 text-right">{textBrightness[key]}</span>
                  <button
                    type="button"
                    onClick={() => setTextBrightness(prev => ({ ...prev, [key]: def }))}
                    className="text-xs text-text-muted hover:text-text-secondary transition-colors px-1"
                    title="Reset to default"
                  >
                    ↺
                  </button>
                </div>
              )
            })}
          </div>
        </details>

        {/* CSS vars textarea */}
        <div className="flex flex-col gap-1">
          <textarea
            value={varsText}
            onChange={e => { setVarsText(e.target.value); setParseError(null) }}
            placeholder={'# YAML or JSON\n--bg-base: "rgba(10, 12, 20, 0.85)"\n--accent: "#4f8ef7"'}
            rows={5}
            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono resize-y"
          />
          <p className="text-[10px] text-text-disabled">
            Accepts flat YAML (--var-name: value) or JSON. Slider updates the vars automatically when a background image is loaded.
          </p>
        </div>

        {/* Background image */}
        <div className="flex flex-col gap-1.5">
          <p className="text-[10px] text-text-secondary">Background image (optional — palette extracted automatically)</p>
          {bgPreview ? (
            <div className="flex items-start gap-2">
              <img
                src={bgPreview}
                className="w-24 h-16 object-cover rounded border border-border flex-shrink-0"
                alt=""
              />
              <div className="flex flex-col gap-1">
                <button
                  type="button"
                  onClick={() => doExtract(bgPreview!)}
                  className="text-xs px-2 py-0.5 rounded border border-border text-accent hover:bg-accent/10 transition-colors"
                >
                  Re-extract
                </button>
                <button
                  type="button"
                  onClick={handleClearBg}
                  className="text-xs px-2 py-0.5 rounded border border-border text-text-muted hover:text-destructive transition-colors"
                >
                  Remove
                </button>
              </div>
            </div>
          ) : (
            <label className="flex items-center gap-2 px-2 py-1.5 rounded border border-dashed border-border cursor-pointer hover:border-accent transition-colors">
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp,image/gif"
                className="hidden"
                onChange={e => {
                  const f = e.target.files?.[0]
                  if (!f) return
                  if (f.size > 10 * 1024 * 1024) { setParseError('Image must be under 10 MB.'); return }
                  handleBgSelect(f)
                  e.target.value = ''
                }}
              />
              <span className="text-xs text-text-secondary">Choose image…</span>
            </label>
          )}
          <p className="text-[10px] text-text-disabled">PNG, JPEG, GIF, or WebP · max 10 MB.</p>
        </div>

        {parseError && <p className="text-xs text-destructive">{parseError}</p>}
      </div>

      <div className="flex gap-2 mt-4">
        <button
          onClick={handleSave}
          disabled={isPending || !name.trim()}
          className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90 disabled:opacity-50"
        >
          {isPending ? 'Saving…' : 'Save theme'}
        </button>
        <button onClick={onCancel} className="text-xs text-text-muted hover:text-text-primary px-3 py-1">
          Cancel
        </button>
      </div>
    </div>
  )
}

function ThemesSection() {
  const qc = useQueryClient()
  const { activeThemeId, setActiveTheme } = useTheme()
  const { data: themes = [], isLoading } = useQuery({
    queryKey: ['themes'],
    queryFn: listThemes,
  })
  const [editing, setEditing] = useState<Theme | 'new' | null>(null)

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

  const [uploading, setUploading] = useState(false)
  const isSavePending = createMutation.isPending || updateMutation.isPending || uploading

  async function handleSave(data: UpsertTheme & { _bgFile?: File }) {
    const { _bgFile, ...themeData } = data
    let bgUrl = themeData.background_url

    if (_bgFile) {
      setUploading(true)
      try {
        const ext = _bgFile.name.split('.').pop()?.toLowerCase() ?? 'bin'
        const safe = new File([_bgFile], `upload.${ext}`, { type: _bgFile.type })
        const form = new FormData()
        form.append('file', safe)
        const resp = await fetch('/api/v1/uploads/images', {
          method: 'POST',
          body: form,
          credentials: 'include',
        })
        if (!resp.ok) throw new Error('Image upload failed')
        const json = await resp.json()
        bgUrl = json.url
      } finally {
        setUploading(false)
      }
    }

    const payload = { ...themeData, background_url: bgUrl }
    if (editing === 'new') {
      await createMutation.mutateAsync(payload)
    } else if (editing != null) {
      await updateMutation.mutateAsync({ id: editing.id, data: payload })
    }
  }

  function handleDelete(theme: Theme) {
    if (!window.confirm(`Delete theme "${theme.name}"?`)) return
    if (activeThemeId === theme.id) setActiveTheme(null)
    deleteMutation.mutate(theme.id)
  }

  const editingInitial: UpsertTheme =
    editing == null || editing === 'new'
      ? { name: '', css_vars: {}, accent_color: null, background_url: null }
      : { name: editing.name, css_vars: editing.css_vars, accent_color: editing.accent_color, background_url: editing.background_url }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <h1 className="text-text-primary font-semibold text-sm">Themes</h1>
        {editing == null && (
          <button
            onClick={() => setEditing('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Theme
          </button>
        )}
      </div>

      {editing != null && (
        <ThemeEditPanel
          key={editing === 'new' ? 'new' : editing.id}
          title={editing === 'new' ? 'New Theme' : `Edit: ${editing.name}`}
          initial={editingInitial}
          onSave={handleSave}
          onCancel={() => setEditing(null)}
          isPending={isSavePending}
        />
      )}

      {isLoading ? (
        <p className="text-text-muted text-xs">Loading…</p>
      ) : themes.length === 0 && editing == null ? (
        <div className="flex flex-col items-center justify-center py-16 gap-2">
          <p className="text-text-muted text-xs">No custom themes defined.</p>
          <p className="text-text-muted text-xs">Themes overlay an accent color and background image on the current base theme.</p>
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
            {themes.map(t => {
              const isActive = activeThemeId === t.id
              return (
                <tr
                  key={t.id}
                  className={`border-b border-border-subtle hover:bg-bg-row-hover ${isActive ? 'bg-accent-muted' : ''}`}
                >
                  <td className="py-1.5 pr-4 text-text-primary font-medium">
                    {isActive && (
                      <span className="inline-block w-1.5 h-1.5 rounded-full bg-accent mr-1.5 mb-0.5" />
                    )}
                    {t.name}
                  </td>
                  <td className="py-1.5 pr-4">
                    {t.accent_color ? (
                      <span
                        className="inline-block w-3.5 h-3.5 rounded-full border border-border"
                        style={{ background: t.accent_color }}
                      />
                    ) : (
                      <span className="text-text-muted">—</span>
                    )}
                  </td>
                  <td className="py-1.5 pr-4 text-text-muted">
                    {t.background_url ? (
                      <img
                        src={t.background_url}
                        alt=""
                        className="h-5 w-8 rounded object-cover border border-border inline-block"
                        onError={e => (e.currentTarget.style.display = 'none')}
                      />
                    ) : '—'}
                  </td>
                  <td className="py-1.5 pl-2">
                    <div className="flex gap-2 justify-end">
                      <button
                        onClick={() => setActiveTheme(isActive ? null : t.id)}
                        className={isActive
                          ? 'text-accent hover:text-text-muted'
                          : 'text-text-muted hover:text-accent'}
                      >
                        {isActive ? 'Remove' : 'Apply'}
                      </button>
                      <button
                        onClick={() => setEditing(t)}
                        className="text-text-muted hover:text-text-primary"
                      >
                        Edit
                      </button>
                      <button
                        onClick={() => handleDelete(t)}
                        className="text-text-muted hover:text-destructive"
                      >
                        Delete
                      </button>
                    </div>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      ) : null}
    </div>
  )
}
