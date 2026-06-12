import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listModels, createModel, deleteModel, updateModel,
  listProviderModels, createProviderModel, updateProviderModel, deleteProviderModel,
  listProviders,
  type Model, type ProviderModel,
} from '../lib/api';
import { Plus, Trash2, Pencil, GitBranch, X, ChevronRight } from 'lucide-react';
import { RelativeTime } from '../components/RelativeTime';
import { useToast } from '../components/Toast';

export default function Models() {
  const toast = useToast();
  const qc = useQueryClient();
  const [showCreateModel, setShowCreateModel] = useState(false);
  const [editModel, setEditModel] = useState<Model | null>(null);
  const [editPM, setEditPM] = useState<ProviderModel | null>(null);
  const [mappingsForModel, setMappingsForModel] = useState<Model | null>(null);
  const [showCreateMapping, setShowCreateMapping] = useState(false);

  const { data: models = [], isLoading: modelsLoading } = useQuery({ queryKey: ['models'], queryFn: listModels });
  const { data: providers = [] } = useQuery({ queryKey: ['providers'], queryFn: listProviders });
  const { data: providerModels = [] } = useQuery({
    queryKey: ['provider-models'],
    queryFn: () => listProviderModels(),
  });

  const providerMap = Object.fromEntries(providers.map(p => [p.id, p.name]));

  // Count mappings per model
  const mappingCountByModel = providerModels.reduce<Record<string, number>>((acc, pm) => {
    acc[pm.model_id] = (acc[pm.model_id] ?? 0) + 1;
    return acc;
  }, {});

  // Filtered mappings for the side panel
  const panelMappings = mappingsForModel
    ? providerModels.filter(pm => pm.model_id === mappingsForModel.id)
    : [];

  const createModelMut = useMutation({
    mutationFn: createModel,
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['models'] }); setShowCreateModel(false); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const updateModelMut = useMutation({
    mutationFn: ({ id, ...data }: { id: string; name: string; is_enabled: boolean }) => updateModel(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['models'] }); setEditModel(null); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const deleteModelMut = useMutation({
    mutationFn: deleteModel,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['models'] }),
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const createPMMut = useMutation({
    mutationFn: createProviderModel,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['provider-models'] });
      setShowCreateMapping(false);
    },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const updatePMMut = useMutation({
    mutationFn: ({ id, ...data }: { id: string; upstream_model_name: string; input_price_per_mtok: number; output_price_per_mtok: number; cache_read_price_per_mtok: number; cache_write_price_per_mtok: number; config: Record<string, unknown> }) =>
      updateProviderModel(id, data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['provider-models'] }); setEditPM(null); },
    onError: (err: Error) => { toast.showError(err.message); },
  });
  const deletePMMut = useMutation({
    mutationFn: deleteProviderModel,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['provider-models'] }),
    onError: (err: Error) => { toast.showError(err.message); },
  });

  return (
    <div className="p-8 space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">Models</h2>
          <p className="text-sm text-gray-500 mt-0.5">Model definitions and provider mappings</p>
        </div>
        <button onClick={() => setShowCreateModel(true)}
          className="flex items-center gap-1.5 px-3 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm">
          <Plus size={15} /> Add Model
        </button>
      </div>

      <div className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
        {modelsLoading ? (
          <div className="p-8 text-center text-gray-500 text-sm">Loading...</div>
        ) : models.length === 0 ? (
          <div className="p-8 text-center text-gray-500 text-sm">No models yet.</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Name</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Enabled</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Mappings</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">ID</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Created</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {models.map(m => {
                const count = mappingCountByModel[m.id] ?? 0;
                const isActive = mappingsForModel?.id === m.id;
                return (
                  <tr key={m.id} className={`border-b border-gray-800/50 hover:bg-gray-800/30 ${isActive ? 'bg-gray-800/40' : ''}`}>
                    <td className="px-5 py-3 text-gray-200 font-medium">{m.name}</td>
                    <td className="px-5 py-3">
                      <span className={`text-xs ${m.is_enabled ? 'text-green-400' : 'text-gray-500'}`}>
                        {m.is_enabled ? 'Yes' : 'No'}
                      </span>
                    </td>
                    <td className="px-5 py-3">
                      <button
                        onClick={() => setMappingsForModel(isActive ? null : m)}
                        className={`group flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all border ${
                          isActive
                            ? 'bg-indigo-600 text-white border-indigo-500 shadow-md shadow-indigo-500/20'
                            : 'bg-gray-800 text-indigo-400 border-gray-700 hover:bg-indigo-600/20 hover:border-indigo-500/50 hover:text-indigo-300'
                        }`}
                        title="View provider mappings"
                      >
                        <GitBranch size={13} />
                        <span>{count} {count === 1 ? 'mapping' : 'mappings'}</span>
                        <ChevronRight size={12} className={`transition-transform ${isActive ? 'rotate-90' : 'group-hover:translate-x-0.5'}`} />
                      </button>
                    </td>
                    <td className="px-5 py-3 text-gray-500 font-mono text-xs">{m.id}</td>
                    <td className="px-5 py-3 text-gray-500 text-xs"><RelativeTime time={m.created_at} /></td>
                    <td className="px-5 py-3 text-right">
                      <div className="flex items-center justify-end gap-2">
                        <button onClick={() => setEditModel(m)}
                          className="p-1.5 text-gray-400 hover:text-gray-200 hover:bg-gray-700 rounded transition-colors">
                          <Pencil size={14} />
                        </button>
                        <button onClick={() => {
                          if (confirm(`Delete model "${m.name}"?`)) deleteModelMut.mutate(m.id);
                        }}
                          className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-950/50 rounded">
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Mappings side panel */}
      {mappingsForModel && (
        <div className="fixed inset-y-0 right-0 w-[500px] bg-gray-950 border-l border-gray-800 shadow-2xl z-40 flex flex-col">
          {/* Panel header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-gray-800">
            <div>
              <h3 className="text-sm font-semibold text-white">Provider Mappings</h3>
              <p className="text-xs text-indigo-400 mt-0.5">{mappingsForModel.name}</p>
            </div>
            <button
              onClick={() => setMappingsForModel(null)}
              className="p-1.5 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition-colors"
            >
              <X size={16} />
            </button>
          </div>

          {/* Add mapping button */}
          <div className="px-5 py-3 border-b border-gray-800/50">
            <button
              onClick={() => setShowCreateMapping(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs"
            >
              <Plus size={13} /> Add Mapping
            </button>
          </div>

          {/* Mappings list */}
          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {panelMappings.length === 0 ? (
              <div className="text-center text-gray-500 text-sm py-8">
                No provider mappings yet.
              </div>
            ) : (
              panelMappings.map(pm => (
                <div key={pm.id} className="bg-gray-900 rounded-lg border border-gray-800 p-4">
                  <div className="flex items-start justify-between mb-2">
                    <div>
                      <p className="text-sm font-medium text-gray-200">
                        {providerMap[pm.provider_id] ?? pm.provider_id.slice(0, 8)}
                      </p>
                      <p className="text-xs text-gray-400 font-mono mt-0.5">{pm.upstream_model_name}</p>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <button
                        onClick={() => setEditPM(pm)}
                        className="p-1.5 text-gray-400 hover:text-gray-200 hover:bg-gray-700 rounded transition-colors"
                      >
                        <Pencil size={13} />
                      </button>
                      <button
                        onClick={() => {
                          if (confirm(`Delete this mapping for "${providerMap[pm.provider_id] ?? pm.provider_id.slice(0, 8)}"?`)) {
                            deletePMMut.mutate(pm.id);
                          }
                        }}
                        className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-950/50 rounded transition-colors"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-gray-500">
                    <span>In: <span className="text-gray-300">{pm.input_price_per_mtok}¢</span> /MTok</span>
                    <span>Out: <span className="text-gray-300">{pm.output_price_per_mtok}¢</span> /MTok</span>
                    {pm.cache_read_price_per_mtok > 0 && (
                      <span>Cache read: <span className="text-gray-300">{pm.cache_read_price_per_mtok}¢</span> /MTok</span>
                    )}
                    {pm.cache_write_price_per_mtok > 0 && (
                      <span>Cache write: <span className="text-gray-300">{pm.cache_write_price_per_mtok}¢</span> /MTok</span>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* Create Model Modal */}
      {showCreateModel && (
        <ModelCreateModal
          onClose={() => setShowCreateModel(false)}
          onSave={name => createModelMut.mutate({ name })}
        />
      )}

      {/* Create Mapping Modal */}
      {showCreateMapping && mappingsForModel && (
        <ProviderModelCreateModal
          models={models}
          providers={providers}
          preselectedModelId={mappingsForModel.id}
          onClose={() => setShowCreateMapping(false)}
          onSave={d => createPMMut.mutate(d)}
        />
      )}

      {/* Edit Model Modal */}
      {editModel && (
        <ModelEditModal
          model={editModel}
          onClose={() => setEditModel(null)}
          onSave={data => updateModelMut.mutate({ id: editModel.id, ...data })}
        />
      )}

      {/* Edit Provider Model Modal */}
      {editPM && (
        <ProviderModelEditModal
          pm={editPM}
          onClose={() => setEditPM(null)}
          onSave={(data: { upstream_model_name: string; input_price_per_mtok: number; output_price_per_mtok: number; cache_read_price_per_mtok: number; cache_write_price_per_mtok: number; config: Record<string, unknown> }) => updatePMMut.mutate({ id: editPM.id, ...data })}
        />
      )}
    </div>
  );
}

function ModelCreateModal({ onClose, onSave }: { onClose: () => void; onSave: (name: string) => void }) {
  const [name, setName] = useState('');
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-sm">
        <h3 className="text-base font-semibold text-white mb-4">Add Model</h3>
        <form onSubmit={e => { e.preventDefault(); onSave(name); }} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Model Name</label>
            <input value={name} onChange={e => setName(e.target.value)} required placeholder="e.g. claude-3-5-sonnet"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
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

function ModelEditModal({ model, onClose, onSave }: {
  model: Model;
  onClose: () => void;
  onSave: (data: { name: string; is_enabled: boolean }) => void;
}) {
  const [name, setName] = useState(model.name);
  const [isEnabled, setIsEnabled] = useState(model.is_enabled);
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-sm">
        <h3 className="text-base font-semibold text-white mb-4">Edit Model</h3>
        <form onSubmit={e => { e.preventDefault(); onSave({ name, is_enabled: isEnabled }); }} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Model Name</label>
            <input value={name} onChange={e => setName(e.target.value)} required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div className="flex items-center gap-2">
            <input type="checkbox" id="model-enabled" checked={isEnabled} onChange={e => setIsEnabled(e.target.checked)}
              className="rounded" />
            <label htmlFor="model-enabled" className="text-sm text-gray-300">Enabled</label>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Cancel</button>
            <button type="submit" className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">Save</button>
          </div>
        </form>
      </div>
    </div>
  );
}

function ProviderModelCreateModal({ models, providers, preselectedModelId, onClose, onSave }: {
  models: Model[];
  providers: { id: string; name: string }[];
  preselectedModelId?: string;
  onClose: () => void;
  onSave: (data: Parameters<typeof createProviderModel>[0]) => void;
}) {
  const [form, setForm] = useState({
    provider_id: '',
    model_id: preselectedModelId ?? '',
    upstream_model_name: '',
    input_price_per_mtok: 0,
    output_price_per_mtok: 0,
    cache_read_price_per_mtok: 0,
    cache_write_price_per_mtok: 0,
  });
  const [configText, setConfigText] = useState('{}');

  function update(key: string, val: string | number) {
    setForm(f => ({ ...f, [key]: val }));
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-lg overflow-y-auto max-h-[90vh]">
        <h3 className="text-base font-semibold text-white mb-4">Add Provider Model Mapping</h3>
        <form onSubmit={e => { e.preventDefault(); let config = {}; try { config = JSON.parse(configText); } catch {} onSave({ ...form, config }); }} className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">Model</label>
              <select value={form.model_id} onChange={e => update('model_id', e.target.value)} required
                disabled={!!preselectedModelId}
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500 disabled:opacity-60">
                <option value="">Select model...</option>
                {models.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">Provider</label>
              <select value={form.provider_id} onChange={e => update('provider_id', e.target.value)} required
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500">
                <option value="">Select provider...</option>
                {providers.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
              </select>
            </div>
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Upstream Model Name</label>
            <input value={form.upstream_model_name} onChange={e => update('upstream_model_name', e.target.value)} required
              placeholder="e.g. claude-3-5-sonnet-20241022"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div className="grid grid-cols-2 gap-3">
            {[
              ['input_price_per_mtok', 'Input ¢/MTok'],
              ['output_price_per_mtok', 'Output ¢/MTok'],
              ['cache_read_price_per_mtok', 'Cache Read ¢/MTok'],
              ['cache_write_price_per_mtok', 'Cache Write ¢/MTok'],
            ].map(([key, label]) => (
              <div key={key}>
                <label className="block text-xs font-medium text-gray-400 mb-1">{label}</label>
                <input type="number" min="0" value={(form as unknown as Record<string, number>)[key]}
                  onChange={e => update(key, parseInt(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
            ))}
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Config (JSON, optional)</label>
            <textarea
              value={configText}
              onChange={e => setConfigText(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-xs font-mono focus:outline-none focus:border-indigo-500"
              placeholder='{"structured_output": "json_object_only"}'
            />
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

function ProviderModelEditModal({ pm, onClose, onSave }: {
  pm: ProviderModel;
  onClose: () => void;
  onSave: (data: {
    upstream_model_name: string;
    input_price_per_mtok: number;
    output_price_per_mtok: number;
    cache_read_price_per_mtok: number;
    cache_write_price_per_mtok: number;
    config: Record<string, unknown>;
  }) => void;
}) {
  const [upstreamModelName, setUpstreamModelName] = useState(pm.upstream_model_name);
  const [inputPrice, setInputPrice] = useState(pm.input_price_per_mtok);
  const [outputPrice, setOutputPrice] = useState(pm.output_price_per_mtok);
  const [cacheReadPrice, setCacheReadPrice] = useState(pm.cache_read_price_per_mtok);
  const [cacheWritePrice, setCacheWritePrice] = useState(pm.cache_write_price_per_mtok);
  const [configText, setConfigText] = useState(JSON.stringify(pm.config ?? {}, null, 2));

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    let config: Record<string, unknown> = {};
    try { config = JSON.parse(configText); } catch { /* use empty */ }
    onSave({
      upstream_model_name: upstreamModelName,
      input_price_per_mtok: inputPrice,
      output_price_per_mtok: outputPrice,
      cache_read_price_per_mtok: cacheReadPrice,
      cache_write_price_per_mtok: cacheWritePrice,
      config,
    });
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-lg overflow-y-auto max-h-[90vh]">
        <h3 className="text-base font-semibold text-white mb-4">Edit Provider Model Mapping</h3>
        <form onSubmit={handleSubmit} className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Upstream Model Name</label>
            <input value={upstreamModelName} onChange={e => setUpstreamModelName(e.target.value)} required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
          </div>
          <div className="grid grid-cols-2 gap-3">
            {([
              ['Input ¢/MTok', inputPrice, setInputPrice],
              ['Output ¢/MTok', outputPrice, setOutputPrice],
              ['Cache Read ¢/MTok', cacheReadPrice, setCacheReadPrice],
              ['Cache Write ¢/MTok', cacheWritePrice, setCacheWritePrice],
            ] as [string, number, (v: number) => void][]).map(([label, val, setter]) => (
              <div key={label}>
                <label className="block text-xs font-medium text-gray-400 mb-1">{label}</label>
                <input type="number" min="0" value={val}
                  onChange={e => setter(parseInt(e.target.value) || 0)}
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
            ))}
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Config (JSON)</label>
            <textarea
              value={configText}
              onChange={e => setConfigText(e.target.value)}
              rows={4}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-xs font-mono focus:outline-none focus:border-indigo-500"
              placeholder='{"structured_output": "json_object_only"}'
            />
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Cancel</button>
            <button type="submit" className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">Save</button>
          </div>
        </form>
      </div>
    </div>
  );
}
