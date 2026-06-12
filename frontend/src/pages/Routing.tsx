import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listModels, listProviderModels, listProviders, listUpstreamKeys,
  createKeyModelAccess, listKeyModelAccess, deleteKeyModelAccess,
  type Model, type ProviderModel, type KeyModelAccess,
} from '../lib/api';
import { Plus, ChevronDown, ChevronRight, Info, Trash2 } from 'lucide-react';
import { useToast } from '../components/Toast';

export default function Routing() {
  const toast = useToast();
  const qc = useQueryClient();
  const [expandedModels, setExpandedModels] = useState<Set<string>>(new Set());
  const [addAccessForPM, setAddAccessForPM] = useState<string | null>(null);

  const { data: models = [], isLoading: modelsLoading } = useQuery({
    queryKey: ['models'],
    queryFn: listModels,
  });
  const { data: providerModels = [], isLoading: pmLoading } = useQuery({
    queryKey: ['provider-models'],
    queryFn: () => listProviderModels(),
  });
  const { data: providers = [] } = useQuery({
    queryKey: ['providers'],
    queryFn: listProviders,
  });
  const { data: upstreamKeys = [] } = useQuery({
    queryKey: ['upstream-keys'],
    queryFn: listUpstreamKeys,
  });
  const { data: keyModelAccess = [] } = useQuery({
    queryKey: ['key-model-access'],
    queryFn: () => listKeyModelAccess(),
  });

  const providerMap = Object.fromEntries(providers.map(p => [p.id, p.name]));
  const keyMap = Object.fromEntries(upstreamKeys.map(k => [k.id, k.alias]));

  // Group provider models by model_id
  const pmByModel: Record<string, ProviderModel[]> = {};
  for (const pm of providerModels) {
    if (!pmByModel[pm.model_id]) pmByModel[pm.model_id] = [];
    pmByModel[pm.model_id].push(pm);
  }

  // Group key_model_access by provider_model_id
  const kmaByPM: Record<string, KeyModelAccess[]> = {};
  for (const kma of keyModelAccess) {
    if (!kmaByPM[kma.provider_model_id]) kmaByPM[kma.provider_model_id] = [];
    kmaByPM[kma.provider_model_id].push(kma);
  }

  const createKMAMut = useMutation({
    mutationFn: createKeyModelAccess,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['key-model-access'] });
      setAddAccessForPM(null);
      toast.showSuccess('Key access added successfully');
    },
    onError: (err: Error) => { toast.showError(err.message); },
  });

  const deleteKMAMut = useMutation({
    mutationFn: deleteKeyModelAccess,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['key-model-access'] });
      toast.showSuccess('Key access removed');
    },
    onError: (err: Error) => { toast.showError(err.message); },
  });

  function toggleModel(id: string) {
    setExpandedModels(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const isLoading = modelsLoading || pmLoading;

  return (
    <div className="p-8 space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-white">Routing Configuration</h2>
        <p className="text-sm text-gray-500 mt-0.5">Configure how model requests are routed to upstream providers and keys</p>
      </div>

      {/* Routing tree */}
      <div className="space-y-3">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500 text-sm">Loading...</div>
        ) : models.length === 0 ? (
          <div className="bg-gray-900 rounded-xl border border-gray-800 p-8 text-center text-gray-500 text-sm">
            No models yet. Create models first.
          </div>
        ) : (
          models.map((model: Model) => {
            const isExpanded = expandedModels.has(model.id);
            const modelPMs = pmByModel[model.id] ?? [];

            return (
              <div key={model.id} className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
                {/* Model row */}
                <button
                  onClick={() => toggleModel(model.id)}
                  className="w-full flex items-center gap-3 px-5 py-3.5 hover:bg-gray-800/50 transition-colors text-left"
                >
                  {isExpanded ? (
                    <ChevronDown size={16} className="text-gray-400 shrink-0" />
                  ) : (
                    <ChevronRight size={16} className="text-gray-400 shrink-0" />
                  )}
                  <span className="font-medium text-white">{model.name}</span>
                  <span className={`text-xs px-1.5 py-0.5 rounded ${model.is_enabled ? 'bg-green-900/50 text-green-400' : 'bg-gray-800 text-gray-500'}`}>
                    {model.is_enabled ? 'enabled' : 'disabled'}
                  </span>
                  <span className="ml-auto text-xs text-gray-500">
                    {modelPMs.length} provider mapping{modelPMs.length !== 1 ? 's' : ''}
                  </span>
                </button>

                {/* Provider models */}
                {isExpanded && (
                  <div className="border-t border-gray-800">
                    {modelPMs.length === 0 ? (
                      <div className="px-10 py-4 text-sm text-gray-500 italic">
                        No provider mappings for this model.
                      </div>
                    ) : (
                      modelPMs.map((pm: ProviderModel, pmIdx: number) => {
                        const pmKeys = (kmaByPM[pm.id] ?? []).slice().sort((a, b) => a.priority - b.priority || b.weight - a.weight);
                        return (
                          <div
                            key={pm.id}
                            className={`${pmIdx > 0 ? 'border-t border-gray-800/60' : ''}`}
                          >
                            {/* Provider model row */}
                            <div className="flex items-center gap-3 px-10 py-3 bg-gray-850">
                              <div className="w-px h-4 bg-gray-700 shrink-0" />
                              <div className="flex items-center gap-2 flex-1 min-w-0">
                                <span className="text-sm text-gray-300 font-mono truncate">{pm.upstream_model_name}</span>
                                <span className="text-gray-600">@</span>
                                <span className="text-sm text-indigo-400">{providerMap[pm.provider_id] ?? pm.provider_id.slice(0, 8)}</span>
                                <span className={`text-xs px-1.5 py-0.5 rounded ml-1 ${pm.is_enabled ? 'bg-green-900/40 text-green-500' : 'bg-gray-800 text-gray-500'}`}>
                                  {pm.is_enabled ? 'enabled' : 'disabled'}
                                </span>
                              </div>
                              <div className="flex items-center gap-4 text-xs text-gray-500 shrink-0">
                                <span>in: {pm.input_price_per_mtok}¢/MTok</span>
                                <span>out: {pm.output_price_per_mtok}¢/MTok</span>
                              </div>
                            </div>

                            {/* Upstream key access entries */}
                            <div className="px-16 pb-3 pt-1 space-y-1">
                              {pmKeys.length === 0 ? (
                                <p className="text-xs text-gray-600 italic">
                                  No keys assigned.
                                </p>
                              ) : (
                                pmKeys.map((kma, kmaIdx) => {
                                  const isLast = kmaIdx === pmKeys.length - 1;
                                  const alias = keyMap[kma.upstream_key_id] ?? kma.upstream_key_id.slice(0, 8);
                                  return (
                                    <div key={kma.id} className="flex items-center gap-2 text-xs text-gray-400">
                                      <span className="text-gray-700 font-mono select-none">
                                        {isLast ? '└─' : '├─'}
                                      </span>
                                      <span className="text-gray-200 font-mono">{alias}</span>
                                      <span className="text-gray-600">—</span>
                                      <span className="text-gray-500">priority: <span className="text-gray-300">{kma.priority}</span></span>
                                      <span className="text-gray-700">,</span>
                                      <span className="text-gray-500">weight: <span className="text-gray-300">{kma.weight}</span></span>
                                      {!kma.is_enabled && (
                                        <span className="text-xs px-1 py-0.5 rounded bg-gray-800 text-gray-500 ml-1">disabled</span>
                                      )}
                                      <button
                                        onClick={() => deleteKMAMut.mutate(kma.id)}
                                        disabled={deleteKMAMut.isPending}
                                        className="ml-1 p-0.5 text-gray-600 hover:text-red-400 transition-colors disabled:opacity-50"
                                        title="Remove key access"
                                      >
                                        <Trash2 size={12} />
                                      </button>
                                    </div>
                                  );
                                })
                              )}
                              <button
                                onClick={() => setAddAccessForPM(pm.id)}
                                className="flex items-center gap-1 text-xs text-indigo-400 hover:text-indigo-300 mt-1 transition-colors"
                              >
                                <Plus size={12} /> Add Key
                              </button>
                            </div>
                          </div>
                        );
                      })
                    )}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* How routing works */}
      <div className="bg-gray-900 rounded-xl border border-gray-800 p-6">
        <div className="flex items-center gap-2 mb-3">
          <Info size={16} className="text-indigo-400" />
          <h3 className="text-sm font-semibold text-white">How Routing Works</h3>
        </div>
        <div className="space-y-2 text-sm text-gray-400">
          <p>
            When a request comes in for a <span className="text-gray-200">model name</span>, xplan:
          </p>
          <ol className="list-decimal list-inside space-y-1.5 pl-2">
            <li>Looks up the virtual <strong className="text-gray-200">model</strong> by name (e.g. <code className="text-xs bg-gray-800 px-1 py-0.5 rounded">claude-3-5-sonnet</code>)</li>
            <li>Finds all <strong className="text-gray-200">provider models</strong> mapped to it — each maps to a specific upstream model name at a provider</li>
            <li>For each provider model, selects eligible <strong className="text-gray-200">upstream keys</strong> via <code className="text-xs bg-gray-800 px-1 py-0.5 rounded">key_model_access</code> (priority + weight)</li>
            <li>Routes the request to the selected key, using <strong className="text-gray-200">priority</strong> (lower = preferred) and <strong className="text-gray-200">weight</strong> (proportional load sharing among equal-priority keys)</li>
          </ol>
          <p className="pt-1 text-gray-500 text-xs">
            Use "Add Key Access" to create a key-model-access entry linking an upstream key to a provider model.
          </p>
        </div>
      </div>

      {/* Add Key Access Modal — scoped to a specific provider_model */}
      {addAccessForPM && (
        <AddKeyModal
          providerModelId={addAccessForPM}
          upstreamKeys={upstreamKeys}
          providerMap={providerMap}
          onClose={() => setAddAccessForPM(null)}
          onSave={d => createKMAMut.mutate(d)}
          isPending={createKMAMut.isPending}
        />
      )}
    </div>
  );
}

function AddKeyModal({
  providerModelId,
  upstreamKeys,
  providerMap,
  onClose,
  onSave,
  isPending,
}: {
  providerModelId: string;
  upstreamKeys: { id: string; provider_id: string; alias: string }[];
  providerMap: Record<string, string>;
  onClose: () => void;
  onSave: (data: { upstream_key_id: string; provider_model_id: string; priority: number; weight: number }) => void;
  isPending: boolean;
}) {
  const [keyId, setKeyId] = useState('');
  const [priority, setPriority] = useState(0);
  const [weight, setWeight] = useState(100);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-xl border border-gray-700 p-6 w-full max-w-sm">
        <h3 className="text-base font-semibold text-white mb-1">Add Key Access</h3>
        <p className="text-xs text-gray-500 mb-4">Select an upstream key and set routing priority/weight.</p>
        <form
          onSubmit={e => { e.preventDefault(); onSave({ upstream_key_id: keyId, provider_model_id: providerModelId, priority, weight }); }}
          className="space-y-4"
        >
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1">Upstream Key</label>
            <select
              value={keyId}
              onChange={e => setKeyId(e.target.value)}
              required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500"
            >
              <option value="">Select key...</option>
              {upstreamKeys.map(k => (
                <option key={k.id} value={k.id}>
                  {k.alias}{providerMap[k.provider_id] ? ` (${providerMap[k.provider_id]})` : ''}
                </option>
              ))}
            </select>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">Priority</label>
              <input type="number" min="0" value={priority}
                onChange={e => setPriority(parseInt(e.target.value) || 0)}
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              <p className="mt-0.5 text-xs text-gray-600">Lower = higher priority</p>
            </div>
            <div>
              <label className="block text-xs font-medium text-gray-400 mb-1">Weight</label>
              <input type="number" min="1" value={weight}
                onChange={e => setWeight(parseInt(e.target.value) || 1)}
                className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              <p className="mt-0.5 text-xs text-gray-600">Proportional load share</p>
            </div>
          </div>

          <div className="flex justify-end gap-2 pt-1">
            <button type="button" onClick={onClose} className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200">Cancel</button>
            <button type="submit" disabled={isPending}
              className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg">
              {isPending ? 'Creating...' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
