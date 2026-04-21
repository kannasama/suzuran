import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { TopNav } from '../components/TopNav'
import { listJobs, cancelJob, type Job } from '../api/jobs'
import { useAuth } from '../contexts/AuthContext'

const STATUS_FILTERS = ['all', 'pending', 'running', 'completed', 'failed', 'cancelled'] as const

function formatDate(iso: string | null): string {
  if (!iso) return '—'
  const d = new Date(iso)
  return d.toLocaleString(undefined, {
    month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', second: '2-digit',
  })
}

function statusColor(status: string): string {
  switch (status) {
    case 'pending':   return 'text-text-muted'
    case 'running':   return 'text-accent'
    case 'completed': return 'text-[color:var(--color-success,#4ade80)]'
    case 'failed':    return 'text-destructive'
    case 'cancelled': return 'text-text-muted'
    default:          return 'text-text-secondary'
  }
}

export default function JobsPage() {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const queryClient = useQueryClient()
  const [statusFilter, setStatusFilter] = useState<string>('all')

  const { data: jobs = [], isLoading } = useQuery({
    queryKey: ['jobs', statusFilter],
    queryFn: () => listJobs({ status: statusFilter === 'all' ? undefined : statusFilter, limit: 200 }),
    refetchInterval: 5_000,
  })

  const cancel = useMutation({
    mutationFn: (id: number) => cancelJob(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['jobs'] }),
  })

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Toolbar */}
        <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-surface border-b border-border flex-shrink-0">
          <span className="text-text-muted text-xs font-medium">Jobs</span>
          <div className="ml-auto flex gap-1">
            {STATUS_FILTERS.map(f => (
              <button
                key={f}
                onClick={() => setStatusFilter(f)}
                className={`text-xs px-2 py-0.5 rounded border transition-colors capitalize ${
                  statusFilter === f
                    ? 'bg-accent/10 border-accent text-accent'
                    : 'bg-bg-panel border-border text-text-muted hover:border-accent hover:text-text-secondary'
                }`}
              >
                {f}
              </button>
            ))}
          </div>
        </div>

        {/* Column headers */}
        <div className="flex items-center gap-2 px-3 py-1 bg-bg-panel border-b border-border text-text-muted text-[11px] uppercase tracking-wider flex-shrink-0">
          <span className="w-14">ID</span>
          <span className="w-32">Type</span>
          <span className="w-20">Status</span>
          <span className="w-8 text-right">Tries</span>
          <span className="flex-1">Payload</span>
          <span className="w-36">Created</span>
          <span className="w-36">Completed</span>
          <span className="w-16">Error</span>
          {isAdmin && <span className="w-16"></span>}
        </div>

        {/* List */}
        <div className="flex-1 overflow-y-auto">
          {isLoading ? (
            <div className="flex items-center justify-center h-32 text-text-muted text-xs">
              Loading…
            </div>
          ) : jobs.length === 0 ? (
            <div className="flex items-center justify-center h-32 text-text-muted text-xs">
              No jobs.
            </div>
          ) : (
            jobs.map((job: Job) => (
              <JobRow key={job.id} job={job} isAdmin={isAdmin} onCancel={() => cancel.mutate(job.id)} />
            ))
          )}
        </div>
      </div>
    </div>
  )
}

function JobRow({ job, isAdmin, onCancel }: { job: Job; isAdmin: boolean; onCancel: () => void }) {
  const [showError, setShowError] = useState(false)
  const payload = job.payload ? JSON.stringify(job.payload) : '—'
  const canCancel = isAdmin && (job.status === 'pending' || job.status === 'running')

  return (
    <div className="flex items-start gap-2 px-3 py-1 border-b border-border-subtle text-xs hover:bg-bg-row-hover">
      <span className="w-14 shrink-0 text-text-muted font-mono">{job.id}</span>
      <span className="w-32 shrink-0 text-text-secondary font-mono truncate">{job.job_type}</span>
      <span className={`w-20 shrink-0 font-medium capitalize ${statusColor(job.status)}`}>{job.status}</span>
      <span className="w-8 shrink-0 text-text-muted text-right">{job.attempts}</span>
      <span className="flex-1 shrink-0 text-text-muted font-mono truncate text-[10px]" title={payload}>
        {payload}
      </span>
      <span className="w-36 shrink-0 text-text-muted">{formatDate(job.created_at)}</span>
      <span className="w-36 shrink-0 text-text-muted">{formatDate(job.completed_at)}</span>
      <span className="w-16 shrink-0">
        {job.error && (
          <button
            className="text-destructive underline decoration-dotted hover:opacity-80"
            onClick={() => setShowError(v => !v)}
            title={job.error ?? undefined}
          >
            error
          </button>
        )}
      </span>
      {isAdmin && (
        <span className="w-16 shrink-0">
          {canCancel && (
            <button
              onClick={onCancel}
              className="text-text-muted hover:text-destructive transition-colors"
            >
              Cancel
            </button>
          )}
        </span>
      )}
      {showError && job.error && (
        <div className="col-span-full w-full px-0 pt-1 pb-0.5 text-destructive font-mono text-[10px] break-all">
          {job.error}
        </div>
      )}
    </div>
  )
}
