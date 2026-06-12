import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useToast } from '../components/Toast';
import { listClientKeys, createClientKey, deleteClientKey, type ClientKeyCreated } from '../lib/api';
import { Plus, Trash2, Copy, CheckCircle, XCircle, Eye, EyeOff } from 'lucide-react';
import { RelativeTime } from '../components/RelativeTime';

function KeyRevealModal({ keyData, onClose }: { keyData: ClientKeyCreated; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  const [visible, setVisible] = useState(true);

  function copyKey() {
    navigator.clipboard.writeText(keyData.key);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-green-800 p-6 w-full max-w-md">
        <div className="flex items-center gap-2 mb-4">
          <CheckCircle size={18} className="text-green-400" />
          <h3 className="text-base font-semibold text-white">Client Key Created</h3>
        </div>
        <div className="bg-yellow-950/30 border border-yellow-800 rounded-lg p-3 mb-4">
          <p className="text-yellow-300 text-xs font-medium">
            Save this key now! It will not be shown again.
          </p>
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-400 mb-1.5">API Key</label>
          <div className="flex items-center gap-2">
            <code className="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-green-300 text-xs font-mono overflow-hidden">
              {visible ? keyData.key : '•'.repeat(40)}
            </code>
            <button onClick={() => setVisible(v => !v)}
              className="p-2 text-gray-400 hover:text-gray-200 bg-gray-800 rounded-lg border border-gray-700">
              {visible ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
            <button onClick={copyKey}
              className="p-2 text-gray-400 hover:text-gray-200 bg-gray-800 rounded-lg border border-gray-700">
              {copied ? <CheckCircle size={14} className="text-green-400" /> : <Copy size={14} />}
            </button>
          </div>
        </div>
        <div className="mt-4 grid grid-cols-2 gap-3 text-xs">
          <div>
            <span className="text-gray-500">Name</span>
            <p className="text-gray-200 font-medium">{keyData.name}</p>
          </div>
          <div>
            <span className="text-gray-500">Prefix</span>
            <p className="text-gray-200 font-mono">{keyData.key_prefix}...</p>
          </div>
        </div>
        <div className="flex justify-end mt-5">
          <button onClick={onClose}
            className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">
            I've saved the key
          </button>
        </div>
      </div>
    </div>
  );
}

function CreateModal({ onClose, onSave }: {
  onClose: () => void;
  onSave: (data: { name: string; access_all_models: boolean; rate_limit_rpm: number | null }) => void;
}) {
  const [name, setName] = useState('');
  const [accessAll, setAccessAll] = useState(true);
  const [rateLimit, setRateLimit] = useState('');

  function submit(e: React.FormEvent) {
    e.preventDefault();
    onSave({
      name,
      access_all_models: accessAll,
      rate_limit_rpm: rateLimit ? parseInt(rateLimit) : null,
    });
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-sm">
        <h3 className="text-base font-semibold text-white mb-4">Create Client Key</h3>
        <form onSubmit={submit} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Key Name</label>
            <input value={name} onChange={e => setName(e.target.value)} required placeholder="e.g. my-app"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Rate Limit (RPM)</label>
            <input type="number" min="1" value={rateLimit} onChange={e => setRateLimit(e.target.value)}
              placeholder="Leave blank for unlimited"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div className="flex items-center gap-2">
            <input type="checkbox" id="access-all" checked={accessAll} onChange={e => setAccessAll(e.target.checked)} />
            <label htmlFor="access-all" className="text-sm text-gray-300">Access all models</label>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Cancel</button>
            <button type="submit" className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">Create</button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default function ClientKeys() {
  const toast = useToast();
  const qc = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [newKey, setNewKey] = useState<ClientKeyCreated | null>(null);
  const { data = [], isLoading, error } = useQuery({ queryKey: ['client-keys'], queryFn: listClientKeys });

  const createMut = useMutation({
    mutationFn: createClientKey,
    onSuccess: (created) => {
      qc.invalidateQueries({ queryKey: ['client-keys'] });
      setShowCreate(false);
      setNewKey(created);
    },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const deleteMut = useMutation({
    mutationFn: deleteClientKey,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['client-keys'] }),
    onError: (err: Error) => { toast.showError(err.message); },
  });

  return (
    <div className="p-8 space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">Client Keys</h2>
          <p className="text-sm text-gray-500 mt-0.5">Manage API keys for your clients</p>
        </div>
        <button onClick={() => setShowCreate(true)}
          className="flex items-center gap-1.5 px-3 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm">
          <Plus size={15} /> Create Key
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
          <div className="p-8 text-center text-gray-500 text-sm">No client keys yet. Create one to get started.</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Name</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Key Prefix</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Access</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Rate Limit</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Status</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Created</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.map(k => (
                <tr key={k.id} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                  <td className="px-5 py-3 text-gray-200 font-medium">{k.name}</td>
                  <td className="px-5 py-3 text-gray-400 font-mono text-xs">{k.key_prefix}...</td>
                  <td className="px-5 py-3 text-gray-400 text-xs">
                    {k.access_all_models ? 'All models' : 'Restricted'}
                  </td>
                  <td className="px-5 py-3 text-gray-400 text-xs">
                    {k.rate_limit_rpm ? `${k.rate_limit_rpm} RPM` : '∞'}
                  </td>
                  <td className="px-5 py-3">
                    {k.is_enabled
                      ? <span className="flex items-center gap-1 text-green-400 text-xs"><CheckCircle size={12} />Active</span>
                      : <span className="flex items-center gap-1 text-gray-500 text-xs"><XCircle size={12} />Disabled</span>
                    }
                  </td>
                  <td className="px-5 py-3 text-gray-500 text-xs"><RelativeTime time={k.created_at} /></td>
                  <td className="px-5 py-3 text-right">
                    <button onClick={() => {
                      if (confirm(`Revoke key "${k.name}"? This cannot be undone.`)) deleteMut.mutate(k.id);
                    }}
                      className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-950/50 rounded">
                      <Trash2 size={14} />
                    </button>
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
      {newKey && (
        <KeyRevealModal keyData={newKey} onClose={() => setNewKey(null)} />
      )}
    </div>
  );
}
