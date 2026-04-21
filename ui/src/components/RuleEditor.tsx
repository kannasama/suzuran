import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createRule, updateRule, type OrgRule, type CreateRuleRequest } from '../api/organizationRules'
import { listLibraries } from '../api/libraries'
import { TemplatePreview } from './TemplatePreview'

interface Props {
  existing?: OrgRule
  onClose: () => void
}

export function RuleEditor({ existing, onClose }: Props) {
  const qc = useQueryClient()
  const { data: libs = [] } = useQuery({ queryKey: ['libraries'], queryFn: listLibraries })

  const [name, setName] = useState(existing?.name ?? '')
  const [libraryId, setLibraryId] = useState<number | null>(existing?.library_id ?? null)
  const [priority, setPriority] = useState(existing?.priority ?? 0)
  const [template, setTemplate] = useState(existing?.path_template ?? '')
  const [enabled, setEnabled] = useState(existing?.enabled ?? true)
  const [error, setError] = useState<string | null>(null)

  const mutation = useMutation({
    mutationFn: (): Promise<OrgRule> => {
      if (existing) {
        return updateRule(existing.id, {
          name, priority, conditions: existing.conditions,
          path_template: template, enabled,
        })
      }
      const req: CreateRuleRequest = { name, library_id: libraryId, priority, conditions: null, path_template: template, enabled }
      return createRule(req)
    },
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['org-rules'] }); onClose() },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'An unexpected error occurred.')
    },
  })

  return (
    <div
      className="fixed inset-0 bg-bg-base/75 flex items-center justify-center z-50"
      onClick={e => { if (e.target === e.currentTarget) onClose() }}
    >
      <div className="bg-bg-surface border border-border rounded w-[560px] flex flex-col" style={{ maxWidth: 'calc(100vw - 2rem)' }}>
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <span className="text-text-primary text-sm font-semibold">{existing ? 'Edit Rule' : 'New Rule'}</span>
          <button onClick={onClose} className="text-text-muted hover:text-text-primary text-sm leading-none" aria-label="Close">×</button>
        </div>

        {/* Form */}
        <form onSubmit={e => { e.preventDefault(); setError(null); mutation.mutate() }} className="flex flex-col gap-3 px-4 py-4">
          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Name</span>
            <input
              type="text" value={name} onChange={e => setName(e.target.value)} autoFocus
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
            />
          </label>

          {!existing && (
            <label className="flex flex-col gap-1">
              <span className="text-text-muted text-xs uppercase tracking-wider">Library</span>
              <select
                value={libraryId ?? ''}
                onChange={e => setLibraryId(e.target.value ? Number(e.target.value) : null)}
                className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
              >
                <option value="">Global (all libraries)</option>
                {libs.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
              </select>
            </label>
          )}

          <label className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Priority</span>
            <div className="flex items-center gap-2">
              <input
                type="number" value={priority} onChange={e => setPriority(Number(e.target.value))}
                className="w-20 bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent"
              />
              <span className="text-text-muted text-xs">Lower = higher priority</span>
            </div>
          </label>

          <div className="flex flex-col gap-1">
            <span className="text-text-muted text-xs uppercase tracking-wider">Path Template</span>
            <input
              type="text" value={template} onChange={e => setTemplate(e.target.value)}
              placeholder="{albumartist}/{date} - {album}/{tracknumber:02} - {title}"
              className="bg-bg-base border border-border text-text-primary text-xs px-2 py-1.5 rounded focus:outline-none focus:border-accent font-mono"
            />
            <TemplatePreview template={template} />
          </div>

          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox" checked={enabled} onChange={e => setEnabled(e.target.checked)}
              className="accent-accent"
            />
            <span className="text-text-primary text-xs">Enabled</span>
          </label>

          {error && <p className="text-destructive text-xs">{error}</p>}

          <div className="flex justify-end gap-2 pt-1">
            <button type="button" onClick={onClose}
              className="text-xs text-text-muted bg-bg-panel border border-border rounded px-3 py-1 hover:text-text-primary hover:border-border">
              Cancel
            </button>
            <button type="submit"
              disabled={!name.trim() || !template.trim() || mutation.isPending}
              className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90">
              {mutation.isPending ? 'Saving…' : 'Save Rule'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
