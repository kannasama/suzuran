import client from './client'

export interface OrgRule {
  id: number
  name: string
  library_id: number | null
  priority: number
  conditions: unknown | null
  path_template: string
  enabled: boolean
  created_at: string
}

export interface CreateRuleRequest {
  name: string
  priority: number
  conditions: unknown | null
  path_template: string
  enabled: boolean
}

export async function listRules(library_id?: number): Promise<OrgRule[]> {
  const res = await client.get<OrgRule[]>('/organization-rules', {
    params: library_id != null ? { library_id } : {}
  })
  return res.data
}

export async function createRule(data: CreateRuleRequest): Promise<OrgRule> {
  const res = await client.post<OrgRule>('/organization-rules', data)
  return res.data
}

export async function updateRule(id: number, data: CreateRuleRequest): Promise<OrgRule> {
  const res = await client.put<OrgRule>(`/organization-rules/${id}`, data)
  return res.data
}

export async function deleteRule(id: number): Promise<void> {
  await client.delete(`/organization-rules/${id}`)
}
