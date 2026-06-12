import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useToast } from '../components/Toast';
import {
  listUpstreamKeys, createUpstreamKey, deleteUpstreamKey,
  listProviders, listUpstreamModels, type UpstreamKey,
} from '../lib/api';
import { List, Plus, Trash2 } from 'lucide-react';
import { RelativeTime } from '../components/RelativeTime';

function statusBadge(status: UpstreamKey['status']) {
  const styles: Record<string, string> = {
    active: 'bg-green-950/50 text-green-400 border-green-900',
    rate_limited: 'bg-yellow-950/50 text-yellow-400 border-yellow-900',
    quota_exceeded: 'bg-red-950/50 text-red-400 border-red-900',
    error: 'bg-red-950/50 text-red-400 border-red-900',
  };
  return `px-2 py-0.5 rounded border text-xs ${styles[status] ?? 'bg-gray-800 text-gray-400 border-gray-700'}`;
}

function CreateModal({ onClose, onSave }: {
  onClose: () => void;
  onSave: (data: { provider_id: string; alias: string; api_key: string }) => void;
}) {
  const { data: providers = [] } = useQuery({ queryKey: ['providers'], queryFn: listProviders });
  const [providerId, setProviderId] = useState('');
  const [alias, setAlias] = useState('');
  const [apiKey, setApiKey] = useState('');

  function submit(e: React.FormEvent) {
    e.preventDefault();
    onSave({ provider_id: providerId, alias, api_key: apiKey });
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-md">
        <h3 className="text-base font-semibold text-white mb-4">Add Upstream Key</h3>
        <form onSubmit={submit} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Provider</label>
            <select value={providerId} onChange={e => setProviderId(e.target.value)} required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500">
              <option value="">Select a provider...</option>
              {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Alias</label>
            <input value={alias} onChange={e => setAlias(e.target.value)} required placeholder="e.g. primary-key"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">API Key</label>
            <input type="password" value={apiKey} onChange={e => setApiKey(e.target.value)} required placeholder="sk-..."
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose}
              className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Cancel</button>
            <button type="submit"
              className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">Create</button>
          </div>
        </form>
      </div>
    </div>
  );
}

function ModelsModal({ keyAlias, keyId, onClose }: {
  keyAlias: string;
  keyId: string;
  onClose: () => void;
}) {
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useState(() => {
    listUpstreamModels(keyId)
      .then(result => {
        setModels(result.models);
        if (result.error) setError(result.error);
      })
      .catch((e: Error) => {
        setError(e.message);
        setModels([]);
      })
      .finally(() => setLoading(false));
  });

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-md max-h-[80vh] flex flex-col"
        onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-base font-semibold text-white">Models — {keyAlias}</h3>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-200 text-xl leading-none">&times;</button>
        </div>

        {loading && (
          <div className="text-center text-gray-500 text-sm py-6">Loading...</div>
        )}

        {!loading && error && (
          <div className="bg-red-950/50 border border-red-900 rounded-lg p-3 text-red-400 text-sm mb-3">
            {error}
          </div>
        )}

        {!loading && !error && models.length === 0 && (
          <div className="text-center text-gray-500 text-sm py-6">No models returned.</div>
        )}

        {!loading && models.length > 0 && (
          <ul className="overflow-y-auto space-y-1.5 flex-1">
            {models.map(m => (
              <li key={m} className="px-3 py-2 bg-gray-800 rounded-lg text-gray-200 text-sm font-mono">
                {m}
              </li>
            ))}
          </ul>
        )}

        <div className="flex justify-end pt-4">
          <button onClick={onClose}
            className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Close</button>
        </div>
      </div>
    </div>
  );
}

export default function UpstreamKeys() {
  const toast = useToast();
  const qc = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [modelsFor, setModelsFor] = useState<{ id: string; alias: string } | null>(null);
  const { data: providers = [] } = useQuery({ queryKey: ['providers'], queryFn: listProviders });
  const { data = [], isLoading, error } = useQuery({ queryKey: ['upstream-keys'], queryFn: listUpstreamKeys });

  const providerMap = Object.fromEntries(providers.map(p => [p.id, p.name]));

  const createMut = useMutation({
    mutationFn: createUpstreamKey,
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['upstream-keys'] }); setShowCreate(false); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const deleteMut = useMutation({
    mutationFn: deleteUpstreamKey,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['upstream-keys'] }),
    onError: (err: Error) => { toast.showError(err.message); },
  });

  return (
    <div className="p-8 space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">Upstream Keys</h2>
          <p className="text-sm text-gray-500 mt-0.5">API keys for upstream providers</p>
        </div>
        <button onClick={() => setShowCreate(true)}
          className="flex items-center gap-1.5 px-3 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm">
          <Plus size={15} /> Add Key
        </button>
      </div>

      {error && (
        <div className="bg-red-950/50 border border-red-900 rounded-xl p-4 text-red-400 text-sm">
          {(error as Error).message}
        </div>
      )}

      <div className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500 text-sm">Loading...</div>
        ) : data.length === 0 ? (
          <div className="p-8 text-center text-gray-500 text-sm">No upstream keys yet.</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Alias</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Provider</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Status</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Enabled</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Created</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.map(k => (
                <tr key={k.id} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                  <td className="px-5 py-3 text-gray-200 font-medium">{k.alias}</td>
                  <td className="px-5 py-3 text-gray-400">{providerMap[k.provider_id] ?? k.provider_id.slice(0, 8)}</td>
                  <td className="px-5 py-3"><span className={statusBadge(k.status)}>{k.status}</span></td>
                  <td className="px-5 py-3">
                    <span className={`text-xs ${k.is_enabled ? 'text-green-400' : 'text-gray-500'}`}>
                      {k.is_enabled ? 'Yes' : 'No'}
                    </span>
                  </td>
                  <td className="px-5 py-3 text-gray-500 text-xs"><RelativeTime time={k.created_at} /></td>
                  <td className="px-5 py-3 text-right">
                    <div className="flex items-center justify-end gap-1">
                      <button
                        onClick={() => setModelsFor({ id: k.id, alias: k.alias })}
                        title="List models"
                        className="p-1.5 text-gray-400 hover:text-indigo-400 hover:bg-indigo-950/50 rounded transition-colors">
                        <List size={14} />
                      </button>
                      <button onClick={() => {
                        if (confirm(`Delete key "${k.alias}"?`)) deleteMut.mutate(k.id);
                      }}
                        className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-950/50 rounded transition-colors">
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {showCreate && (
        <CreateModal onClose={() => setShowCreate(false)} onSave={d => createMut.mutate(d)} />
      )}

      {modelsFor && (
        <ModelsModal
          keyId={modelsFor.id}
          keyAlias={modelsFor.alias}
          onClose={() => setModelsFor(null)}
        />
      )}
    </div>
  );
}
