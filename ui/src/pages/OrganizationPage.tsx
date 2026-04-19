import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { RuleEditor } from '../components/RuleEditor'
import { listRules, deleteRule, type OrgRule } from '../api/organizationRules'

export default function OrganizationPage() {
  const qc = useQueryClient()
  const { data: rules = [], isLoading } = useQuery({ queryKey: ['org-rules'], queryFn: () => listRules() })
  const [editing, setEditing] = useState<OrgRule | null | 'new'>(null)

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteRule(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['org-rules'] }),
  })

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <main className="flex-1 overflow-y-auto p-6">
        <div className="flex items-center justify-between mb-5">
          <h1 className="text-text-primary font-semibold text-sm">Organization Rules</h1>
          <button
            onClick={() => setEditing('new')}
            className="text-xs text-bg-base bg-accent rounded px-3 py-1 font-medium hover:opacity-90"
          >
            + New Rule
          </button>
        </div>

        {isLoading ? (
          <p className="text-text-muted text-xs">Loading…</p>
        ) : rules.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 gap-2">
            <p className="text-text-muted text-xs">No organization rules defined.</p>
            <p className="text-text-muted text-[10px]">Rules determine how files are named and organized on disk.</p>
          </div>
        ) : (
          <table className="w-full text-xs border-collapse">
            <thead>
              <tr className="border-b border-border text-text-muted text-[9px] uppercase tracking-wider">
                <th className="text-left pb-2 pr-4 font-medium">Priority</th>
                <th className="text-left pb-2 pr-4 font-medium">Name</th>
                <th className="text-left pb-2 pr-4 font-medium">Library</th>
                <th className="text-left pb-2 pr-4 font-medium">Template</th>
                <th className="text-left pb-2 pr-4 font-medium">Enabled</th>
                <th className="pb-2"></th>
              </tr>
            </thead>
            <tbody>
              {rules.map(rule => (
                <tr key={rule.id} className="border-b border-border-subtle hover:bg-bg-panel">
                  <td className="py-1.5 pr-4 text-text-muted">{rule.priority}</td>
                  <td className="py-1.5 pr-4 text-text-primary font-medium">{rule.name}</td>
                  <td className="py-1.5 pr-4 text-text-muted">{rule.library_id == null ? 'Global' : `#${rule.library_id}`}</td>
                  <td className="py-1.5 pr-4 font-mono text-text-muted max-w-xs truncate">{rule.path_template}</td>
                  <td className="py-1.5 pr-4">
                    <span className={`text-[9px] px-1.5 py-0.5 rounded ${rule.enabled ? 'bg-accent-muted text-accent' : 'bg-bg-panel text-text-muted'}`}>
                      {rule.enabled ? 'on' : 'off'}
                    </span>
                  </td>
                  <td className="py-1.5 pl-2">
                    <div className="flex gap-2 justify-end">
                      <button onClick={() => setEditing(rule)} className="text-text-muted hover:text-text-primary">Edit</button>
                      <button
                        onClick={() => { if (window.confirm(`Delete rule "${rule.name}"?`)) deleteMutation.mutate(rule.id) }}
                        className="text-text-muted hover:text-destructive"
                      >Delete</button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </main>

      {editing != null && (
        <RuleEditor
          existing={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  )
}
