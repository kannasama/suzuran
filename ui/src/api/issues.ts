import client from './client'
import type { Issue } from '../types/issue'

export interface ListIssuesParams {
  library_id?: number
  type?: string
  include_dismissed?: boolean
}

export const issuesApi = {
  list(params: ListIssuesParams = {}): Promise<Issue[]> {
    const q: Record<string, string> = {}
    if (params.library_id != null) q.library_id = String(params.library_id)
    if (params.type) q.type = params.type
    if (params.include_dismissed) q.include_dismissed = 'true'
    return client.get<Issue[]>('/issues', { params: q }).then(r => r.data)
  },

  count(): Promise<number> {
    return client.get<{ count: number }>('/issues/count').then(r => r.data.count)
  },

  dismiss(id: number): Promise<void> {
    return client.post(`/issues/${id}/dismiss`).then(() => undefined)
  },

  rescan(trackIds: number[]): Promise<{ refreshed: number; errors: string[] }> {
    return client
      .post<{ refreshed: number; errors: string[] }>('/issues/rescan', { track_ids: trackIds })
      .then(r => r.data)
  },
}
