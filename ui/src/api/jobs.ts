import client from './client'

export interface Job {
  id: number
  job_type: string
  status: string
  payload: unknown
  result: unknown | null
  priority: number
  attempts: number
  error: string | null
  created_at: string
  started_at: string | null
  completed_at: string | null
}

export async function listJobs(params?: { status?: string; limit?: number }): Promise<Job[]> {
  const res = await client.get('/jobs', { params })
  return res.data
}

export async function getJob(id: number): Promise<Job> {
  const res = await client.get(`/jobs/${id}`)
  return res.data
}

export async function cancelJob(id: number): Promise<void> {
  await client.post(`/jobs/${id}/cancel`)
}
