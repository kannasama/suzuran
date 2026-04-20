import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { EncodingProfileForm } from '../components/EncodingProfileForm'
import { ArtProfileForm } from '../components/ArtProfileForm'
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
import type { EncodingProfile, UpsertEncodingProfile } from '../types/encodingProfile'
import type { ArtProfile, UpsertArtProfile } from '../types/artProfile'

type ActiveTab = 'encoding' | 'art'

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
        </div>

        <div className="p-6">
          {activeTab === 'encoding' && <EncodingProfilesSection />}
          {activeTab === 'art' && <ArtProfilesSection />}
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
          <p className="text-text-muted text-[10px] uppercase tracking-wider mb-3">
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
          <p className="text-text-muted text-[10px]">Profiles configure the output codec and quality for transcoding.</p>
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
          <p className="text-text-muted text-[10px] uppercase tracking-wider mb-3">
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
          <p className="text-text-muted text-[10px]">Profiles configure cover art resizing and recompression.</p>
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
