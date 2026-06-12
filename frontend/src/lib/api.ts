const API_BASE = '/admin/api';

function getToken(): string {
  return localStorage.getItem('xplan_admin_token') || '';
}

export async function apiFetch<T = unknown>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${getToken()}`,
      ...options?.headers,
    },
  });
  if (!res.ok) {
    const text = await res.text();
    let message = `HTTP ${res.status}`;
    try {
      const json = JSON.parse(text);
      message = json.error?.message || json.error || json.message || text;
    } catch {
      message = text || message;
    }
    // Translate common DB constraint errors
    if (message.includes('duplicate key value violates unique constraint')) {
      if (message.includes('provider_model')) message = 'This provider-model mapping already exists.';
      else if (message.includes('key_model_access')) message = 'This key-model access already exists.';
      else if (message.includes('models_name')) message = 'A model with this name already exists.';
      else if (message.includes('providers_name')) message = 'A provider with this name already exists.';
      else message = 'This record already exists (duplicate).';
    }
    throw new Error(message);
  }
  // 204 No Content
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

// ── Dashboard ──────────────────────────────────────────────────────────────
export interface DashboardOverview {
  from: string;
  to: string;
  total_requests: number;
  total_cost_cents: number;
  total_input_tokens: number;
  total_output_tokens: number;
  by_model: UsageSummary[];
}

export interface UsageSummary {
  model_name: string;
  provider_name: string;
  total_requests: number;
  total_cost_cents: number;
  total_input_tokens: number;
  total_output_tokens: number;
}

// ── Providers ──────────────────────────────────────────────────────────────
export type ApiFormat = 'openai_compatible' | 'anthropic' | 'bedrock' | 'responses';

export interface Provider {
  id: string;
  name: string;
  base_url: string;
  api_format: ApiFormat;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export async function listProviders(): Promise<Provider[]> {
  return apiFetch('/providers');
}

export async function createProvider(data: { name: string; base_url: string; api_format: ApiFormat }): Promise<Provider> {
  return apiFetch('/providers', { method: 'POST', body: JSON.stringify(data) });
}

export async function updateProvider(id: string, data: { name: string; base_url: string; api_format: ApiFormat; is_enabled: boolean }): Promise<Provider> {
  return apiFetch(`/providers/${id}`, { method: 'PUT', body: JSON.stringify(data) });
}

export async function deleteProvider(id: string): Promise<void> {
  return apiFetch(`/providers/${id}`, { method: 'DELETE' });
}

// ── Upstream Keys ──────────────────────────────────────────────────────────
export type KeyStatus = 'active' | 'rate_limited' | 'quota_exceeded' | 'error';

export interface UpstreamKey {
  id: string;
  provider_id: string;
  alias: string;
  is_enabled: boolean;
  status: KeyStatus;
  created_at: string;
  updated_at: string;
}

export async function listUpstreamKeys(): Promise<UpstreamKey[]> {
  return apiFetch('/upstream-keys');
}

export async function createUpstreamKey(data: { provider_id: string; alias: string; api_key: string }): Promise<UpstreamKey> {
  return apiFetch('/upstream-keys', { method: 'POST', body: JSON.stringify(data) });
}

export async function deleteUpstreamKey(id: string): Promise<void> {
  return apiFetch(`/upstream-keys/${id}`, { method: 'DELETE' });
}

export async function listUpstreamModels(keyId: string): Promise<{ models: string[]; error?: string }> {
  return apiFetch(`/upstream-keys/${keyId}/models`);
}

// ── Models ─────────────────────────────────────────────────────────────────
export interface Model {
  id: string;
  name: string;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProviderModel {
  id: string;
  provider_id: string;
  model_id: string;
  upstream_model_name: string;
  input_price_per_mtok: number;
  output_price_per_mtok: number;
  cache_read_price_per_mtok: number;
  cache_write_price_per_mtok: number;
  config: Record<string, unknown>;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export async function listModels(): Promise<Model[]> {
  return apiFetch('/models');
}

export async function createModel(data: { name: string }): Promise<Model> {
  return apiFetch('/models', { method: 'POST', body: JSON.stringify(data) });
}

export async function deleteModel(id: string): Promise<void> {
  return apiFetch(`/models/${id}`, { method: 'DELETE' });
}

export async function updateModel(id: string, data: { name: string; is_enabled: boolean }): Promise<Model> {
  return apiFetch(`/models/${id}`, { method: 'PUT', body: JSON.stringify(data) });
}

export async function listProviderModels(modelId?: string): Promise<ProviderModel[]> {
  const q = modelId ? `?model_id=${modelId}` : '';
  return apiFetch(`/provider-models${q}`);
}

export async function createProviderModel(data: {
  provider_id: string;
  model_id: string;
  upstream_model_name: string;
  input_price_per_mtok: number;
  output_price_per_mtok: number;
  cache_read_price_per_mtok: number;
  cache_write_price_per_mtok: number;
  config?: Record<string, unknown>;
}): Promise<ProviderModel> {
  return apiFetch('/provider-models', { method: 'POST', body: JSON.stringify(data) });
}

export async function deleteProviderModel(id: string): Promise<void> {
  return apiFetch(`/provider-models/${id}`, { method: 'DELETE' });
}

export async function updateProviderModel(id: string, data: {
  upstream_model_name: string;
  input_price_per_mtok: number;
  output_price_per_mtok: number;
  cache_read_price_per_mtok: number;
  cache_write_price_per_mtok: number;
  config: Record<string, unknown>;
}): Promise<ProviderModel> {
  return apiFetch(`/provider-models/${id}`, { method: 'PUT', body: JSON.stringify(data) });
}

export async function createKeyModelAccess(data: {
  upstream_key_id: string;
  provider_model_id: string;
  priority: number;
  weight: number;
}): Promise<unknown> {
  return apiFetch('/key-model-access', { method: 'POST', body: JSON.stringify(data) });
}

export interface KeyModelAccess {
  id: string;
  upstream_key_id: string;
  provider_model_id: string;
  priority: number;
  weight: number;
  is_enabled: boolean;
  created_at: string;
}

export async function listKeyModelAccess(providerModelId?: string): Promise<KeyModelAccess[]> {
  const params = providerModelId ? `?provider_model_id=${providerModelId}` : '';
  return apiFetch(`/key-model-access${params}`);
}

export async function deleteKeyModelAccess(id: string): Promise<void> {
  return apiFetch(`/key-model-access/${id}`, { method: 'DELETE' });
}

// ── Client Keys ────────────────────────────────────────────────────────────
export interface ClientKey {
  id: string;
  name: string;
  key_prefix: string;
  access_all_models: boolean;
  is_enabled: boolean;
  rate_limit_rpm: number | null;
  created_at: string;
  updated_at: string;
}

export interface ClientKeyCreated extends ClientKey {
  key: string;
}

export async function listClientKeys(): Promise<ClientKey[]> {
  return apiFetch('/client-keys');
}

export async function createClientKey(data: { name: string; access_all_models?: boolean; rate_limit_rpm?: number | null }): Promise<ClientKeyCreated> {
  return apiFetch('/client-keys', { method: 'POST', body: JSON.stringify(data) });
}

export async function deleteClientKey(id: string): Promise<void> {
  return apiFetch(`/client-keys/${id}`, { method: 'DELETE' });
}

// ── Usage ──────────────────────────────────────────────────────────────────
export interface UsageLog {
  id: string;
  client_key_id: string | null;
  model_name: string;
  provider_name: string;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  cost_cents: number;
  latency_ms: number;
  status: string;
  created_at: string;
}

export interface UsageQueryParams {
  from?: string;
  to?: string;
  model?: string;
  provider?: string;
  limit?: number;
  offset?: number;
}

export async function queryUsage(params: UsageQueryParams = {}): Promise<UsageLog[]> {
  const q = new URLSearchParams();
  if (params.from) q.set('from', params.from);
  if (params.to) q.set('to', params.to);
  if (params.model) q.set('model', params.model);
  if (params.provider) q.set('provider', params.provider);
  if (params.limit !== undefined) q.set('limit', String(params.limit));
  if (params.offset !== undefined) q.set('offset', String(params.offset));
  const qs = q.toString();
  return apiFetch(`/usage${qs ? '?' + qs : ''}`);
}

export interface UsageBreakdown {
  group_id: string;
  group_name: string;
  model_name: string;
  provider_name: string;
  total_requests: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_cents: number;
}

export async function usageByUpstreamKey(from?: string, to?: string): Promise<UsageBreakdown[]> {
  const params = new URLSearchParams();
  if (from) params.set('from', from);
  if (to) params.set('to', to);
  const qs = params.toString();
  return apiFetch(`/usage/by-upstream-key${qs ? '?' + qs : ''}`);
}

export async function usageByClientKey(from?: string, to?: string): Promise<UsageBreakdown[]> {
  const params = new URLSearchParams();
  if (from) params.set('from', from);
  if (to) params.set('to', to);
  const qs = params.toString();
  return apiFetch(`/usage/by-client-key${qs ? '?' + qs : ''}`);
}
