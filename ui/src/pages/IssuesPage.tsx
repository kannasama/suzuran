import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { issuesApi } from '../api/issues'
import { listLibraries } from '../api/libraries'
import type { Issue } from '../types/issue'

const SEVERITY_COLORS: Record<Issue['severity'], string> = {
  high:   'text-destructive',
  medium: 'text-yellow-400',
  low:    'text-text-muted',
}

const SEVERITY_LABELS: Record<Issue['severity'], string> = {
  high:   'High',
  medium: 'Medium',
  low:    'Low',
}

const TYPE_LABELS: Record<Issue['issue_type'], string> = {
  missing_file:        'Missing file',
  bad_audio_properties: 'Bad audio properties',
  untagged:            'Untagged',
  duplicate_mb_id:     'Duplicate MB ID',
}

export default function IssuesPage() {
  const qc = useQueryClient()
  const [libraryFilter, setLibraryFilter] = useState<string>('')
  const [typeFilter, setTypeFilter] = useState<string>('')
  const [showDismissed, setShowDismissed] = useState(false)

  const { data: libraries = [] } = useQuery({
    queryKey: ['libraries'],
    queryFn: listLibraries,
  })

  const { data: issues = [], isLoading } = useQuery({
    queryKey: ['issues', libraryFilter, typeFilter, showDismissed],
    queryFn: () =>
      issuesApi.list({
        library_id: libraryFilter ? Number(libraryFilter) : undefined,
        type: typeFilter || undefined,
        include_dismissed: showDismissed,
      }),
    refetchInterval: 30_000,
  })

  const dismiss = useMutation({
    mutationFn: (id: number) => issuesApi.dismiss(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['issues'] })
      qc.invalidateQueries({ queryKey: ['issues-count'] })
    },
  })

  const rescan = useMutation({
    mutationFn: (trackIds: number[]) => issuesApi.rescan(trackIds),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['issues'] })
      qc.invalidateQueries({ queryKey: ['issues-count'] })
      qc.invalidateQueries({ queryKey: ['library-tracks'] })
    },
  })

  const libraryById = Object.fromEntries(libraries.map(l => [l.id, l]))

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Toolbar */}
        <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-bg-surface flex-shrink-0 flex-wrap">
          <span className="text-xs text-text-muted">
            {isLoading ? 'Loading…' : `${issues.length} issue${issues.length !== 1 ? 's' : ''}`}
          </span>

          {/* Library filter */}
          <select
            value={libraryFilter}
            onChange={e => setLibraryFilter(e.target.value)}
            className="text-xs bg-bg-panel border border-border rounded px-2 py-0.5 text-text-secondary"
          >
            <option value="">All Libraries</option>
            {libraries.map(l => (
              <option key={l.id} value={String(l.id)}>{l.name}</option>
            ))}
          </select>

          {/* Type filter */}
          <select
            value={typeFilter}
            onChange={e => setTypeFilter(e.target.value)}
            className="text-xs bg-bg-panel border border-border rounded px-2 py-0.5 text-text-secondary"
          >
            <option value="">All Types</option>
            <option value="missing_file">Missing file</option>
            <option value="bad_audio_properties">Bad audio properties</option>
            <option value="untagged">Untagged</option>
            <option value="duplicate_mb_id">Duplicate MB ID</option>
          </select>

          <label className="flex items-center gap-1.5 text-xs text-text-muted cursor-pointer ml-auto">
            <input
              type="checkbox"
              checked={showDismissed}
              onChange={e => setShowDismissed(e.target.checked)}
              className="accent-accent"
            />
            Show dismissed
          </label>
        </div>

        {/* Issue list */}
        <div className="flex-1 overflow-y-auto">
          {isLoading ? (
            <div className="p-4 text-text-muted text-xs">Loading…</div>
          ) : issues.length === 0 ? (
            <div className="p-8 text-center text-text-muted text-sm">No issues</div>
          ) : (
            <table className="w-full text-xs border-collapse">
              <thead className="sticky top-0 bg-bg-surface border-b border-border">
                <tr>
                  <th className="text-left px-3 py-1.5 text-text-muted font-normal w-16">Severity</th>
                  <th className="text-left px-3 py-1.5 text-text-muted font-normal w-36">Type</th>
                  <th className="text-left px-3 py-1.5 text-text-muted font-normal">Track</th>
                  <th className="text-left px-3 py-1.5 text-text-muted font-normal w-32">Library</th>
                  <th className="text-left px-3 py-1.5 text-text-muted font-normal">Detail</th>
                  <th className="px-3 py-1.5 w-32"></th>
                </tr>
              </thead>
              <tbody>
                {issues.map(issue => (
                  <IssueRow
                    key={issue.id}
                    issue={issue}
                    libraryName={libraryById[issue.library_id]?.name ?? `#${issue.library_id}`}
                    onDismiss={() => dismiss.mutate(issue.id)}
                    onRescan={() => issue.track_id != null && rescan.mutate([issue.track_id])}
                    isDismissing={dismiss.isPending && dismiss.variables === issue.id}
                    isRescanning={rescan.isPending}
                  />
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  )
}

function IssueRow({
  issue,
  libraryName,
  onDismiss,
  onRescan,
  isDismissing,
  isRescanning,
}: {
  issue: Issue
  libraryName: string
  onDismiss: () => void
  onRescan: () => void
  isDismissing: boolean
  isRescanning: boolean
}) {
  const trackPath = issue.track_id != null
    ? `#${issue.track_id}`
    : '—'

  return (
    <tr className={`border-b border-border hover:bg-bg-hover transition-colors ${issue.dismissed ? 'opacity-50' : ''}`}>
      <td className={`px-3 py-1.5 font-medium ${SEVERITY_COLORS[issue.severity]}`}>
        {SEVERITY_LABELS[issue.severity]}
      </td>
      <td className="px-3 py-1.5 text-text-secondary">
        {TYPE_LABELS[issue.issue_type]}
      </td>
      <td className="px-3 py-1.5 text-text-muted font-mono truncate max-w-xs" title={issue.detail ?? undefined}>
        {trackPath}
      </td>
      <td className="px-3 py-1.5 text-text-muted truncate">{libraryName}</td>
      <td className="px-3 py-1.5 text-text-muted truncate max-w-sm" title={issue.detail ?? undefined}>
        {issue.detail ?? '—'}
      </td>
      <td className="px-3 py-1.5">
        <div className="flex gap-1 justify-end">
          {issue.issue_type === 'bad_audio_properties' && issue.track_id != null && (
            <button
              onClick={onRescan}
              disabled={isRescanning}
              className="px-2 py-0.5 text-xs border border-border rounded hover:border-accent hover:text-accent disabled:opacity-40"
            >
              Rescan
            </button>
          )}
          {issue.issue_type === 'missing_file' && (
            <button
              onClick={onDismiss}
              disabled={isDismissing}
              className="px-2 py-0.5 text-xs border border-border rounded hover:border-destructive hover:text-destructive disabled:opacity-40"
            >
              Dismiss
            </button>
          )}
          {issue.issue_type !== 'missing_file' && (
            <button
              onClick={onDismiss}
              disabled={isDismissing}
              className="px-2 py-0.5 text-xs border border-border rounded hover:border-border hover:text-text-secondary disabled:opacity-40 text-text-muted"
            >
              Dismiss
            </button>
          )}
        </div>
      </td>
    </tr>
  )
}
