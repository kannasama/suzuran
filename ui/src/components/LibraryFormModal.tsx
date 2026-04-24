import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createLibrary,
  updateLibrary,
  type Library,
  type CreateLibraryInput,
  type UpdateLibraryInput,
} from '../api/libraries'
import {
  listLibraryProfiles,
  createLibraryProfile,
  deleteLibraryProfile,
} from '../api/libraryProfiles'
import { listEncodingProfiles } from '../api/encodingProfiles'
import { listRules } from '../api/organizationRules'
import type { LibraryProfile } from '../types/libraryProfile'

interface Props {
  /** When provided, the modal is in edit mode for this library */
  library?: Library
  onClose: () => void
}

const FORMATS = ['flac', 'aac', 'mp3', 'opus', 'wav'] as const
const TAG_ENCODINGS = [
  { value: 'utf8', label: 'UTF-8 (default)' },
  { value: 'sjis', label: 'Shift-JIS (Japanese legacy)' },
] as const

export function LibraryFormModal({ library, onClose }: Props) {
  const isEdit = library !== undefined
  const queryClient = useQueryClient()

  const [name, setName] = useState(library?.name ?? '')
  const [rootPath, setRootPath] = useState('')
  const [format, setFormat] = useState<string>('flac')
  const [tagEncoding, setTagEncoding] = useState(library?.tag_encoding ?? 'utf8')
  const [organizationRuleId, setOrganizationRuleId] = useState<number | null>(
    library?.organization_rule_id ?? null,
  )
  const [scanEnabled, setScanEnabled] = useState(library?.scan_enabled ?? true)
  const [scanIntervalSecs, setScanIntervalSecs] = useState(
    library?.scan_interval_secs ?? 3600,
  )
  const [autoOrganize, setAutoOrganize] = useState(
    library?.auto_organize_on_ingest ?? false,
  )
  const [isDefault, setIsDefault] = useState(library?.is_default ?? false)
  const [maintenanceIntervalSecs, setMaintenanceIntervalSecs] = useState<string>(
    library?.maintenance_interval_secs != null ? String(library.maintenance_interval_secs) : '',
  )

  const [error, setError] = useState<string | null>(null)

  // Profile add form state
  const [showAddProfile, setShowAddProfile] = useState(false)
  const [newProfileDirName, setNewProfileDirName] = useState('')
  const [newProfileEncodingId, setNewProfileEncodingId] = useState<number | ''>('')
  const [newProfileIncludeOnSubmit, setNewProfileIncludeOnSubmit] = useState(true)
  const [newProfileAboveHz, setNewProfileAboveHz] = useState<string>('')
  const [addProfileError, setAddProfileError] = useState<string | null>(null)

  const { data: encodingProfiles = [] } = useQuery({
    queryKey: ['encoding-profiles'],
    queryFn: listEncodingProfiles,
  })

  const { data: orgRules = [] } = useQuery({
    queryKey: ['org-rules'],
    queryFn: () => listRules(),
  })

  const { data: fetchedProfiles = [], refetch: refetchProfiles } = useQuery({
    queryKey: ['library-profiles', library?.id],
    queryFn: () => listLibraryProfiles(library!.id),
    enabled: isEdit && library != null,
  })

  // Local ordered copy of profiles — supports visual reordering without an API
  const [profileOrder, setProfileOrder] = useState<LibraryProfile[]>([])
  // Sync fetched profiles into local order state (only when the fetched list changes length)
  const libraryProfiles = profileOrder.length > 0 && profileOrder.length === fetchedProfiles.length
    ? profileOrder
    : fetchedProfiles

  const createMutation = useMutation({
    mutationFn: (input: CreateLibraryInput) => createLibrary(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
      onClose()
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'An unexpected error occurred.')
    },
  })

  const updateMutation = useMutation({
    mutationFn: (input: UpdateLibraryInput) => updateLibrary(library!.id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['libraries'] })
      onClose()
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'An unexpected error occurred.')
    },
  })

  const addProfileMutation = useMutation({
    mutationFn: () => {
      if (newProfileEncodingId === '') throw new Error('Select an encoding profile.')
      return createLibraryProfile({
        library_id: library!.id,
        encoding_profile_id: newProfileEncodingId as number,
        derived_dir_name: newProfileDirName.trim(),
        include_on_submit: newProfileIncludeOnSubmit,
        auto_include_above_hz: newProfileAboveHz.trim() !== '' ? Number(newProfileAboveHz) : null,
      })
    },
    onSuccess: () => {
      setProfileOrder([])
      refetchProfiles()
      setShowAddProfile(false)
      setNewProfileDirName('')
      setNewProfileEncodingId('')
      setNewProfileIncludeOnSubmit(true)
      setNewProfileAboveHz('')
      setAddProfileError(null)
    },
    onError: (err: unknown) => {
      setAddProfileError(err instanceof Error ? err.message : 'Failed to add profile.')
    },
  })

  const deleteProfileMutation = useMutation({
    mutationFn: (id: number) => deleteLibraryProfile(id),
    onSuccess: () => { setProfileOrder([]); refetchProfiles() },
  })

  function moveProfileUp(index: number) {
    if (index === 0) return
    const next = [...libraryProfiles]
    ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
    setProfileOrder(next)
  }

  function moveProfileDown(index: number) {
    if (index === libraryProfiles.length - 1) return
    const next = [...libraryProfiles]
    ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
    setProfileOrder(next)
  }

  const isPending = createMutation.isPending || updateMutation.isPending

  const isSubmitDisabled =
    isPending ||
    name.trim() === '' ||
    (!isEdit && rootPath.trim() === '')

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)

    if (isEdit) {
      updateMutation.mutate({
        name: name.trim(),
        scan_enabled: scanEnabled,
        scan_interval_secs: scanIntervalSecs,
        auto_organize_on_ingest: autoOrganize,
        tag_encoding: tagEncoding,
        organization_rule_id: organizationRuleId,
        is_default: isDefault,
        maintenance_interval_secs: maintenanceIntervalSecs.trim() !== ''
          ? Number(maintenanceIntervalSecs)
          : null,
      })
    } else {
      createMutation.mutate({
        name: name.trim(),
        root_path: rootPath.trim(),
        format,
        organization_rule_id: organizationRuleId,
      })
    }
  }

  function getEncodingProfileName(id: number) {
    const p = encodingProfiles.find(ep => ep.id === id)
    return p ? `${p.name} (${p.codec.toUpperCase()})` : `Profile #${id}`
  }

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div
        className="bg-bg-surface border border-border rounded w-[560px] flex flex-col overflow-y-auto"
        style={{ maxHeight: 'calc(100vh - 4rem)', maxWidth: 'calc(100vw - 2rem)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
          <span className="text-text-primary text-sm font-semibold">
            {isEdit ? 'Edit Library' : 'Add Library'}
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
        <form onSubmit={handleSubmit} className="flex flex-col gap-3 px-4 py-4">
          {/* Name */}
          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Name</span>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder="My Music"
              autoFocus
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            />
          </label>

          {/* Create-only fields */}
          {!isEdit && (
            <>
              <label className="flex flex-col gap-1">
                <span className="text-text-muted text-xs uppercase tracking-wider">Root Path</span>
                <input
                  type="text"
                  value={rootPath}
                  onChange={e => setRootPath(e.target.value)}
                  placeholder="/mnt/music/flac"
                  className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-text-muted text-xs uppercase tracking-wider">Format</span>
                <select
                  value={format}
                  onChange={e => setFormat(e.target.value)}
                  className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                >
                  {FORMATS.map(f => (
                    <option key={f} value={f}>{f.toUpperCase()}</option>
                  ))}
                </select>
              </label>
            </>
          )}

          {/* Tag Encoding */}
          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Tag Encoding</span>
            <select
              value={tagEncoding}
              onChange={e => setTagEncoding(e.target.value)}
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            >
              {TAG_ENCODINGS.map(enc => (
                <option key={enc.value} value={enc.value}>{enc.label}</option>
              ))}
            </select>
          </label>

          {/* Organization Rule */}
          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Organization Rule</span>
            <select
              value={organizationRuleId ?? ''}
              onChange={e => setOrganizationRuleId(e.target.value === '' ? null : Number(e.target.value))}
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            >
              <option value="">— None —</option>
              {orgRules.map(r => (
                <option key={r.id} value={r.id}>{r.name}</option>
              ))}
            </select>
          </label>

          {/* Edit-only fields */}
          {isEdit && (
            <>
              {/* Scan settings */}
              <div className="flex gap-3 items-start">
                <label className="flex flex-col gap-1 flex-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Scan Interval (sec)</span>
                  <input
                    type="number"
                    min={60}
                    value={scanIntervalSecs}
                    onChange={e => setScanIntervalSecs(Number(e.target.value))}
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                  />
                </label>
                <label className="flex flex-col gap-1 shrink-0">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Scan</span>
                  <button
                    type="button"
                    onClick={() => setScanEnabled(v => !v)}
                    className={`text-xs rounded px-3 py-1.5 font-medium border transition-colors ${
                      scanEnabled
                        ? 'bg-accent text-bg-base border-transparent hover:opacity-90'
                        : 'bg-transparent text-text-secondary border-border hover:border-accent hover:text-text-primary'
                    }`}
                  >
                    {scanEnabled ? 'Enabled' : 'Disabled'}
                  </button>
                </label>
                <label className="flex flex-col gap-1 shrink-0">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Auto-Organize</span>
                  <button
                    type="button"
                    onClick={() => setAutoOrganize(v => !v)}
                    className={`text-xs rounded px-3 py-1.5 font-medium border transition-colors ${
                      autoOrganize
                        ? 'bg-accent text-bg-base border-transparent hover:opacity-90'
                        : 'bg-transparent text-text-secondary border-border hover:border-accent hover:text-text-primary'
                    }`}
                  >
                    {autoOrganize ? 'On' : 'Off'}
                  </button>
                </label>
              </div>

              {/* Maintenance + default */}
              <div className="flex gap-3 items-start">
                <label className="flex flex-col gap-1 flex-1">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Maintenance Interval (sec)</span>
                  <input
                    type="number"
                    min={60}
                    value={maintenanceIntervalSecs}
                    onChange={e => setMaintenanceIntervalSecs(e.target.value)}
                    placeholder="Disabled"
                    className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent placeholder:text-text-muted/50"
                  />
                </label>
                <label className="flex flex-col gap-1 shrink-0">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Default</span>
                  <button
                    type="button"
                    onClick={() => setIsDefault(v => !v)}
                    className={`text-xs rounded px-3 py-1.5 font-medium border transition-colors ${
                      isDefault
                        ? 'bg-accent text-bg-base border-transparent hover:opacity-90'
                        : 'bg-transparent text-text-secondary border-border hover:border-accent hover:text-text-primary'
                    }`}
                  >
                    {isDefault ? 'Yes' : 'No'}
                  </button>
                </label>
              </div>

              {/* Profiles section */}
              <div className="flex flex-col gap-2 pt-1">
                <div className="flex items-center justify-between">
                  <span className="text-text-muted text-xs uppercase tracking-wider">Profiles</span>
                  {!showAddProfile && (
                    <button
                      type="button"
                      onClick={() => setShowAddProfile(true)}
                      className="text-xs text-bg-base bg-accent rounded px-2 py-0.5 font-medium hover:opacity-90"
                    >
                      + Add Profile
                    </button>
                  )}
                </div>

                {/* Profile list */}
                {libraryProfiles.length > 0 && (
                  <div className="flex flex-col gap-1">
                    {libraryProfiles.map((p: LibraryProfile, i: number) => (
                      <div
                        key={p.id}
                        className="flex items-center gap-2 px-2 py-1.5 bg-bg-panel border border-border rounded text-xs"
                      >
                        <div className="flex flex-col shrink-0">
                          <button
                            type="button"
                            onClick={() => moveProfileUp(i)}
                            disabled={i === 0}
                            className="text-text-muted hover:text-text-primary disabled:opacity-30 text-xs px-0.5 leading-none"
                            title="Move up"
                          >
                            ▲
                          </button>
                          <button
                            type="button"
                            onClick={() => moveProfileDown(i)}
                            disabled={i === libraryProfiles.length - 1}
                            className="text-text-muted hover:text-text-primary disabled:opacity-30 text-xs px-0.5 leading-none"
                            title="Move down"
                          >
                            ▼
                          </button>
                        </div>
                        <span className="font-mono text-text-primary flex-1 truncate">{p.derived_dir_name || '—'}</span>
                        <span className="text-text-muted shrink-0">{getEncodingProfileName(p.encoding_profile_id)}</span>
                        {p.include_on_submit && (
                          <span className="shrink-0 text-[10px] bg-accent/20 text-accent rounded px-1.5 py-0.5">submit</span>
                        )}
                        {p.auto_include_above_hz != null && (
                          <span className="shrink-0 text-[10px] text-text-muted">≥{(p.auto_include_above_hz / 1000).toFixed(0)}kHz</span>
                        )}
                        <button
                          type="button"
                          onClick={() => deleteProfileMutation.mutate(p.id)}
                          disabled={deleteProfileMutation.isPending}
                          className="text-text-muted hover:text-destructive shrink-0 transition-colors"
                          title="Delete profile"
                        >
                          ✕
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                {libraryProfiles.length === 0 && !showAddProfile && (
                  <p className="text-text-muted text-xs italic">No profiles — derived formats not configured.</p>
                )}

                {/* Add profile inline form */}
                {showAddProfile && (
                  <div className="flex flex-col gap-2 p-3 bg-bg-panel border border-border rounded">
                    <div className="flex gap-2">
                      <label className="flex flex-col gap-1 flex-1">
                        <span className="text-text-muted text-[10px] uppercase tracking-wider">Dir Name</span>
                        <input
                          type="text"
                          value={newProfileDirName}
                          onChange={e => setNewProfileDirName(e.target.value)}
                          placeholder="mp3-320"
                          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
                        />
                      </label>
                      <label className="flex flex-col gap-1 flex-1">
                        <span className="text-text-muted text-[10px] uppercase tracking-wider">Encoding Profile</span>
                        <select
                          value={newProfileEncodingId}
                          onChange={e => setNewProfileEncodingId(e.target.value === '' ? '' : Number(e.target.value))}
                          className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                        >
                          <option value="">— Select —</option>
                          {encodingProfiles.map(ep => (
                            <option key={ep.id} value={ep.id}>{ep.name} ({ep.codec.toUpperCase()})</option>
                          ))}
                        </select>
                      </label>
                    </div>
                    <div className="flex gap-2 items-end">
                      {encodingProfiles.find(ep => ep.id === newProfileEncodingId)?.codec === 'flac' && (
                        <label className="flex flex-col gap-1 flex-1">
                          <span className="text-text-muted text-[10px] uppercase tracking-wider">Min Sample Rate (Hz)</span>
                          <input
                            type="number"
                            value={newProfileAboveHz}
                            onChange={e => setNewProfileAboveHz(e.target.value)}
                            placeholder="Optional — e.g. 88200"
                            className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
                          />
                        </label>
                      )}
                      <label className="flex flex-col gap-1 shrink-0">
                        <span className="text-text-muted text-[10px] uppercase tracking-wider">Include on Submit</span>
                        <button
                          type="button"
                          onClick={() => setNewProfileIncludeOnSubmit(v => !v)}
                          className={`text-xs rounded px-3 py-1.5 font-medium border transition-colors ${
                            newProfileIncludeOnSubmit
                              ? 'bg-accent text-bg-base border-transparent hover:opacity-90'
                              : 'bg-transparent text-text-secondary border-border hover:border-accent hover:text-text-primary'
                          }`}
                        >
                          {newProfileIncludeOnSubmit ? 'Yes' : 'No'}
                        </button>
                      </label>
                    </div>
                    {addProfileError && (
                      <p className="text-destructive text-xs">{addProfileError}</p>
                    )}
                    <div className="flex gap-2 justify-end">
                      <button
                        type="button"
                        onClick={() => { setShowAddProfile(false); setAddProfileError(null) }}
                        className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary"
                      >
                        Cancel
                      </button>
                      <button
                        type="button"
                        onClick={() => addProfileMutation.mutate()}
                        disabled={addProfileMutation.isPending || newProfileEncodingId === ''}
                        className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
                      >
                        {addProfileMutation.isPending ? 'Saving…' : 'Save'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </>
          )}

          {/* Inline error */}
          {error && (
            <p className="text-destructive text-xs">{error}</p>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitDisabled}
              className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
            >
              {isPending ? 'Saving…' : isEdit ? 'Save' : 'Add Library'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
