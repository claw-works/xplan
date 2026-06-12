import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { queryUsage, usageByUpstreamKey, usageByClientKey, type UsageLog, type UsageBreakdown } from '../lib/api';
import { RelativeTime } from '../components/RelativeTime';

// cost_cents is stored in micro-cents. Divide by 1_000_000 to get cents,
// then by 100 to get the base currency unit (e.g. USD).
function fmtCost(microCents: number) {
  return (microCents / 100_000_000).toFixed(6);
}

function fmtTokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function statusBadge(status: string) {
  if (status === 'success') return 'bg-green-950/50 text-green-400 border-green-900';
  if (status === 'error') return 'bg-red-950/50 text-red-400 border-red-900';
  return 'bg-gray-800 text-gray-400 border-gray-700';
}

type Tab = 'logs' | 'by-upstream-key' | 'by-client-key';

function DateRangeBar({
  from, to, setFrom, setTo, onApply,
}: {
  from: string;
  to: string;
  setFrom: (v: string) => void;
  setTo: (v: string) => void;
  onApply: () => void;
}) {
  return (
    <div className="bg-gray-900 rounded-xl border border-gray-800 p-4 flex flex-wrap items-end gap-3">
      <div>
        <label className="block text-xs font-medium text-gray-400 mb-1">From</label>
        <input type="datetime-local" value={from} onChange={e => setFrom(e.target.value)}
          className="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
      </div>
      <div>
        <label className="block text-xs font-medium text-gray-400 mb-1">To</label>
        <input type="datetime-local" value={to} onChange={e => setTo(e.target.value)}
          className="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
      </div>
      <button onClick={onApply}
        className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm transition-colors">
        Apply
      </button>
    </div>
  );
}

function BreakdownTable({ data, isLoading, error, groupLabel }: {
  data: UsageBreakdown[];
  isLoading: boolean;
  error: unknown;
  groupLabel: string;
}) {
  return (
    <div className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
      {isLoading ? (
        <div className="p-8 text-center text-gray-500 text-sm">Loading...</div>
      ) : error ? (
        <div className="p-8 text-center text-red-400 text-sm">{(error as Error).message}</div>
      ) : data.length === 0 ? (
        <div className="p-8 text-center text-gray-500 text-sm">No data found for the selected period.</div>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-gray-800">
              <th className="text-left px-4 py-3 text-gray-400 font-medium">{groupLabel}</th>
              <th className="text-left px-4 py-3 text-gray-400 font-medium">Model</th>
              <th className="text-left px-4 py-3 text-gray-400 font-medium">Provider</th>
              <th className="text-right px-4 py-3 text-gray-400 font-medium">Requests</th>
              <th className="text-right px-4 py-3 text-gray-400 font-medium">In Tokens</th>
              <th className="text-right px-4 py-3 text-gray-400 font-medium">Out Tokens</th>
              <th className="text-right px-4 py-3 text-gray-400 font-medium">Cost</th>
            </tr>
          </thead>
          <tbody>
            {data.map((row, i) => (
              <tr key={i} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                <td className="px-4 py-2.5 text-gray-200 text-xs font-medium">{row.group_name}</td>
                <td className="px-4 py-2.5 text-gray-200 text-xs">{row.model_name}</td>
                <td className="px-4 py-2.5 text-gray-400 text-xs">{row.provider_name}</td>
                <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{row.total_requests.toLocaleString()}</td>
                <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{fmtTokens(row.total_input_tokens)}</td>
                <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{fmtTokens(row.total_output_tokens)}</td>
                <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{fmtCost(row.total_cost_cents)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

export default function Usage() {
  const now = new Date();
  const weekAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);

  const [activeTab, setActiveTab] = useState<Tab>('logs');

  // Logs tab state
  const [from, setFrom] = useState(() => weekAgo.toISOString().slice(0, 16));
  const [to, setTo] = useState(() => now.toISOString().slice(0, 16));
  const [model, setModel] = useState('');
  const [provider, setProvider] = useState('');
  const [limit, setLimit] = useState(100);
  const [offset, setOffset] = useState(0);
  const [submitted, setSubmitted] = useState({
    from: weekAgo.toISOString(),
    to: now.toISOString(),
    model: '',
    provider: '',
    limit: 100,
    offset: 0,
  });

  // Breakdown tabs shared date range
  const [bdFrom, setBdFrom] = useState(() => weekAgo.toISOString().slice(0, 16));
  const [bdTo, setBdTo] = useState(() => now.toISOString().slice(0, 16));
  const [bdSubmitted, setBdSubmitted] = useState({
    from: weekAgo.toISOString(),
    to: now.toISOString(),
  });

  const { data: logsData = [], isLoading: logsLoading, error: logsError, isFetching: logsFetching } = useQuery<UsageLog[]>({
    queryKey: ['usage', submitted],
    queryFn: () => queryUsage({
      from: submitted.from,
      to: submitted.to,
      model: submitted.model || undefined,
      provider: submitted.provider || undefined,
      limit: submitted.limit,
      offset: submitted.offset,
    }),
    enabled: activeTab === 'logs',
  });

  const { data: upstreamData = [], isLoading: upstreamLoading, error: upstreamError } = useQuery<UsageBreakdown[]>({
    queryKey: ['usage-by-upstream-key', bdSubmitted],
    queryFn: () => usageByUpstreamKey(bdSubmitted.from, bdSubmitted.to),
    enabled: activeTab === 'by-upstream-key',
  });

  const { data: clientData = [], isLoading: clientLoading, error: clientError } = useQuery<UsageBreakdown[]>({
    queryKey: ['usage-by-client-key', bdSubmitted],
    queryFn: () => usageByClientKey(bdSubmitted.from, bdSubmitted.to),
    enabled: activeTab === 'by-client-key',
  });

  function applyFilters(e: React.FormEvent) {
    e.preventDefault();
    setSubmitted({
      from: from ? new Date(from).toISOString() : weekAgo.toISOString(),
      to: to ? new Date(to).toISOString() : now.toISOString(),
      model,
      provider,
      limit,
      offset: 0,
    });
    setOffset(0);
  }

  function applyBdFilters() {
    setBdSubmitted({
      from: bdFrom ? new Date(bdFrom).toISOString() : weekAgo.toISOString(),
      to: bdTo ? new Date(bdTo).toISOString() : now.toISOString(),
    });
  }

  function prevPage() {
    const newOffset = Math.max(0, offset - limit);
    setOffset(newOffset);
    setSubmitted(s => ({ ...s, offset: newOffset }));
  }

  function nextPage() {
    const newOffset = offset + limit;
    setOffset(newOffset);
    setSubmitted(s => ({ ...s, offset: newOffset }));
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: 'logs', label: 'Logs' },
    { id: 'by-upstream-key', label: 'By Upstream Key' },
    { id: 'by-client-key', label: 'By Client Key' },
  ];

  return (
    <div className="p-8 space-y-5">
      <div>
        <h2 className="text-xl font-semibold text-white">Usage</h2>
        <p className="text-sm text-gray-500 mt-0.5">Query and filter usage history</p>
      </div>

      {/* Tab bar */}
      <div className="flex gap-1 bg-gray-900 rounded-xl border border-gray-800 p-1 w-fit">
        {tabs.map(tab => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-indigo-600 text-white'
                : 'text-gray-400 hover:text-gray-200 hover:bg-gray-800'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Logs tab */}
      {activeTab === 'logs' && (
        <>
          <form onSubmit={applyFilters} className="bg-gray-900 rounded-xl border border-gray-800 p-4">
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">From</label>
                <input type="datetime-local" value={from} onChange={e => setFrom(e.target.value)}
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">To</label>
                <input type="datetime-local" value={to} onChange={e => setTo(e.target.value)}
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">Model</label>
                <input value={model} onChange={e => setModel(e.target.value)} placeholder="Filter by model..."
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-400 mb-1">Provider</label>
                <input value={provider} onChange={e => setProvider(e.target.value)} placeholder="Filter by provider..."
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-sm focus:outline-none focus:border-indigo-500" />
              </div>
            </div>
            <div className="flex items-center justify-between mt-3">
              <div className="flex items-center gap-2">
                <label className="text-xs text-gray-400">Limit</label>
                <select value={limit} onChange={e => setLimit(Number(e.target.value))}
                  className="px-2 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-gray-100 text-xs focus:outline-none focus:border-indigo-500">
                  {[25, 50, 100, 250].map(v => <option key={v} value={v}>{v}</option>)}
                </select>
              </div>
              <button type="submit"
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-sm transition-colors">
                Apply Filters
              </button>
            </div>
          </form>

          {logsError && (
            <div className="bg-red-950/50 border border-red-900 rounded-xl p-4 text-red-400 text-sm">
              {(logsError as Error).message}
            </div>
          )}

          <div className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
            {logsLoading ? (
              <div className="p-8 text-center text-gray-500 text-sm">Loading...</div>
            ) : logsData.length === 0 ? (
              <div className="p-8 text-center text-gray-500 text-sm">No usage logs found for the selected filters.</div>
            ) : (
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    <th className="text-left px-4 py-3 text-gray-400 font-medium">Time</th>
                    <th className="text-left px-4 py-3 text-gray-400 font-medium">Model</th>
                    <th className="text-left px-4 py-3 text-gray-400 font-medium">Provider</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium">In</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium" title="Cache read tokens">Cache R</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium" title="Cache write tokens">Cache W</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium">Out</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium">Cost</th>
                    <th className="text-right px-4 py-3 text-gray-400 font-medium">Latency</th>
                    <th className="text-left px-4 py-3 text-gray-400 font-medium">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {logsData.map(log => (
                    <tr key={log.id} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                      <td className="px-4 py-2.5 text-gray-500 text-xs whitespace-nowrap">
                        <RelativeTime time={log.created_at} />
                      </td>
                      <td className="px-4 py-2.5 text-gray-200 text-xs">{log.model_name}</td>
                      <td className="px-4 py-2.5 text-gray-400 text-xs">{log.provider_name}</td>
                      <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{log.input_tokens.toLocaleString()}</td>
                      <td className="px-4 py-2.5 text-right text-emerald-400 text-xs">{log.cache_read_tokens ? log.cache_read_tokens.toLocaleString() : '—'}</td>
                      <td className="px-4 py-2.5 text-right text-amber-400 text-xs">{log.cache_write_tokens ? log.cache_write_tokens.toLocaleString() : '—'}</td>
                      <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{log.output_tokens.toLocaleString()}</td>
                      <td className="px-4 py-2.5 text-right text-gray-300 text-xs">{fmtCost(log.cost_cents)}</td>
                      <td className="px-4 py-2.5 text-right text-gray-400 text-xs">{log.latency_ms}ms</td>
                      <td className="px-4 py-2.5">
                        <span className={`px-2 py-0.5 rounded border text-xs ${statusBadge(log.status)}`}>
                          {log.status}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>

          {!logsLoading && (
            <div className="flex items-center justify-between text-sm text-gray-500">
              <span>
                {logsData.length === 0 ? 'No results' : `Showing ${offset + 1}–${offset + logsData.length}`}
                {logsFetching && <span className="ml-2 text-indigo-400">Refreshing...</span>}
              </span>
              <div className="flex gap-2">
                <button onClick={prevPage} disabled={offset === 0}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed rounded-lg text-xs">
                  Previous
                </button>
                <button onClick={nextPage} disabled={logsData.length < limit}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed rounded-lg text-xs">
                  Next
                </button>
              </div>
            </div>
          )}
        </>
      )}

      {/* By Upstream Key tab */}
      {activeTab === 'by-upstream-key' && (
        <>
          <DateRangeBar from={bdFrom} to={bdTo} setFrom={setBdFrom} setTo={setBdTo} onApply={applyBdFilters} />
          <BreakdownTable
            data={upstreamData}
            isLoading={upstreamLoading}
            error={upstreamError}
            groupLabel="Upstream Key"
          />
        </>
      )}

      {/* By Client Key tab */}
      {activeTab === 'by-client-key' && (
        <>
          <DateRangeBar from={bdFrom} to={bdTo} setFrom={setBdFrom} setTo={setBdTo} onApply={applyBdFilters} />
          <BreakdownTable
            data={clientData}
            isLoading={clientLoading}
            error={clientError}
            groupLabel="Client Key"
          />
        </>
      )}
    </div>
  );
}
