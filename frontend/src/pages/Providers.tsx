import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useToast } from '../components/Toast';
import {
  listProviders, createProvider, updateProvider, deleteProvider,
  type Provider, type ApiFormat,
} from '../lib/api';
import { Plus, Pencil, Trash2, CheckCircle, XCircle } from 'lucide-react';

const FORMAT_OPTIONS: ApiFormat[] = ['openai_compatible', 'anthropic', 'bedrock', 'responses'];

function ProviderModal({
  initial,
  onClose,
  onSave,
}: {
  initial?: Provider;
  onClose: () => void;
  onSave: (data: { name: string; base_url: string; api_format: ApiFormat; is_enabled?: boolean }) => void;
}) {
  const [name, setName] = useState(initial?.name ?? '');
  const [baseUrl, setBaseUrl] = useState(initial?.base_url ?? '');
  const [format, setFormat] = useState<ApiFormat>(initial?.api_format ?? 'openai_compatible');
  const [enabled, setEnabled] = useState(initial?.is_enabled ?? true);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    onSave({ name, base_url: baseUrl, api_format: format, is_enabled: enabled });
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-md">
        <h3 className="text-base font-semibold text-white mb-4">
          {initial ? 'Edit Provider' : 'Add Provider'}
        </h3>
        <form onSubmit={submit} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Name</label>
            <input value={name} onChange={e => setName(e.target.value)} required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Base URL</label>
            <input value={baseUrl} onChange={e => setBaseUrl(e.target.value)} required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">API Format</label>
            <select value={format} onChange={e => setFormat(e.target.value as ApiFormat)}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500">
              {FORMAT_OPTIONS.map(f => <option key={f} value={f}>{f}</option>)}
            </select>
          </div>
          {initial && (
            <div className="flex items-center gap-2">
              <input type="checkbox" id="enabled" checked={enabled} onChange={e => setEnabled(e.target.checked)}
                className="rounded" />
              <label htmlFor="enabled" className="text-sm text-gray-300">Enabled</label>
            </div>
          )}
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose}
              className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200 transition-colors">Cancel</button>
            <button type="submit"
              className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg transition-colors">
              {initial ? 'Save' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default function Providers() {
  const toast = useToast();
  const qc = useQueryClient();
  const [modal, setModal] = useState<'create' | Provider | null>(null);
  const { data = [], isLoading, error } = useQuery({ queryKey: ['providers'], queryFn: listProviders });

  const createMut = useMutation({
    mutationFn: createProvider,
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['providers'] }); setModal(null); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const updateMut = useMutation({
    mutationFn: ({ id, ...data }: { id: string; name: string; base_url: string; api_format: ApiFormat; is_enabled: boolean }) =>
      updateProvider(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['providers'] }); setModal(null); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const deleteMut = useMutation({
    mutationFn: deleteProvider,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
    onError: (err: Error) => { toast.showError(err.message); },
  });

  return (
    <div className="p-8 space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">Providers</h2>
          <p className="text-sm text-gray-500 mt-0.5">Manage LLM providers</p>
        </div>
        <button onClick={() => setModal('create')}
          className="flex items-center gap-1.5 px-3 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm transition-colors">
          <Plus size={15} /> Add Provider
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
          <div className="p-8 text-center text-gray-500 text-sm">No providers yet. Add one to get started.</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Name</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Base URL</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Format</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Status</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.map(p => (
                <tr key={p.id} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                  <td className="px-5 py-3 text-gray-200 font-medium">{p.name}</td>
                  <td className="px-5 py-3 text-gray-400 font-mono text-xs">{p.base_url}</td>
                  <td className="px-5 py-3">
                    <span className="px-2 py-0.5 bg-gray-800 text-gray-300 rounded text-xs">{p.api_format}</span>
                  </td>
                  <td className="px-5 py-3">
                    {p.is_enabled
                      ? <span className="flex items-center gap-1 text-green-400 text-xs"><CheckCircle size={12} />Enabled</span>
                      : <span className="flex items-center gap-1 text-gray-500 text-xs"><XCircle size={12} />Disabled</span>
                    }
                  </td>
                  <td className="px-5 py-3 text-right">
                    <div className="flex items-center justify-end gap-2">
                      <button onClick={() => setModal(p)}
                        className="p-1.5 text-gray-400 hover:text-gray-200 hover:bg-gray-700 rounded transition-colors">
                        <Pencil size={14} />
                      </button>
                      <button onClick={() => {
                        if (confirm(`Delete provider "${p.name}"?`)) deleteMut.mutate(p.id);
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

      {modal === 'create' && (
        <ProviderModal
          onClose={() => setModal(null)}
          onSave={d => createMut.mutate(d)}
        />
      )}
      {modal && modal !== 'create' && (
        <ProviderModal
          initial={modal as Provider}
          onClose={() => setModal(null)}
          onSave={d => updateMut.mutate({ id: (modal as Provider).id, ...d as { name: string; base_url: string; api_format: ApiFormat; is_enabled: boolean } })}
        />
      )}
    </div>
  );
}
