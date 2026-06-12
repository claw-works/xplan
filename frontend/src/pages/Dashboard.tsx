import { useQuery } from '@tanstack/react-query';
import { apiFetch, type DashboardOverview } from '../lib/api';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import { Activity, DollarSign, Zap, TrendingUp } from 'lucide-react';

function StatCard({ label, value, icon: Icon, sub }: {
  label: string;
  value: string | number;
  icon: React.ElementType;
  sub?: string;
}) {
  return (
    <div className="bg-gray-900 rounded-xl border border-gray-800 p-5">
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm text-gray-400">{label}</span>
        <div className="p-2 bg-indigo-600/20 rounded-lg">
          <Icon size={16} className="text-indigo-400" />
        </div>
      </div>
      <div className="text-2xl font-semibold text-white">{value}</div>
      {sub && <div className="text-xs text-gray-500 mt-1">{sub}</div>}
    </div>
  );
}

// cost_cents is now stored in micro-cents. Divide by 1_000_000 to get cents,
// then by 100 to get the base currency unit (e.g. USD).
function fmtCost(microCents: number) {
  return (microCents / 100_000_000).toFixed(6);
}

function fmtTokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export default function Dashboard() {
  const { data, isLoading, error } = useQuery<DashboardOverview>({
    queryKey: ['dashboard'],
    queryFn: () => apiFetch('/dashboard'),
    refetchInterval: 30_000,
  });

  if (isLoading) {
    return (
      <div className="p-8">
        <div className="animate-pulse space-y-4">
          <div className="h-8 bg-gray-800 rounded w-48" />
          <div className="grid grid-cols-4 gap-4">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="h-28 bg-gray-800 rounded-xl" />
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-8">
        <div className="bg-red-950/50 border border-red-900 rounded-xl p-4 text-red-400 text-sm">
          Failed to load dashboard: {(error as Error).message}
        </div>
      </div>
    );
  }

  const chartData = (data?.by_model ?? []).map(m => ({
    name: m.model_name,
    requests: m.total_requests,
    cost: +(m.total_cost_cents / 100_000_000).toFixed(6),
  }));

  return (
    <div className="p-8 space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-white">Dashboard</h2>
        <p className="text-sm text-gray-500 mt-0.5">Last 24 hours</p>
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          label="Total Requests"
          value={data?.total_requests ?? 0}
          icon={Activity}
          sub="past 24h"
        />
        <StatCard
          label="Total Cost"
          value={fmtCost(data?.total_cost_cents ?? 0)}
          icon={DollarSign}
          sub="past 24h"
        />
        <StatCard
          label="Input Tokens"
          value={fmtTokens(data?.total_input_tokens ?? 0)}
          icon={Zap}
          sub="past 24h"
        />
        <StatCard
          label="Output Tokens"
          value={fmtTokens(data?.total_output_tokens ?? 0)}
          icon={TrendingUp}
          sub="past 24h"
        />
      </div>

      {chartData.length > 0 && (
        <div className="bg-gray-900 rounded-xl border border-gray-800 p-5">
          <h3 className="text-sm font-medium text-gray-300 mb-4">Requests by Model</h3>
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={chartData} margin={{ top: 0, right: 0, left: -20, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="#1f2937" />
              <XAxis dataKey="name" tick={{ fontSize: 11, fill: '#9ca3af' }} />
              <YAxis tick={{ fontSize: 11, fill: '#9ca3af' }} />
              <Tooltip
                contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8, fontSize: 12 }}
                labelStyle={{ color: '#e5e7eb' }}
                itemStyle={{ color: '#818cf8' }}
              />
              <Bar dataKey="requests" fill="#6366f1" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {(data?.by_model?.length ?? 0) > 0 && (
        <div className="bg-gray-900 rounded-xl border border-gray-800 overflow-hidden">
          <div className="px-5 py-3 border-b border-gray-800">
            <h3 className="text-sm font-medium text-gray-300">Breakdown by Model</h3>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Model</th>
                <th className="text-left px-5 py-3 text-gray-400 font-medium">Provider</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Requests</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Cost</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">In Tokens</th>
                <th className="text-right px-5 py-3 text-gray-400 font-medium">Out Tokens</th>
              </tr>
            </thead>
            <tbody>
              {data?.by_model.map((row, i) => (
                <tr key={i} className="border-b border-gray-800/50 hover:bg-gray-800/30">
                  <td className="px-5 py-3 text-gray-200">{row.model_name}</td>
                  <td className="px-5 py-3 text-gray-400">{row.provider_name}</td>
                  <td className="px-5 py-3 text-right text-gray-200">{row.total_requests}</td>
                  <td className="px-5 py-3 text-right text-gray-200">{fmtCost(row.total_cost_cents)}</td>
                  <td className="px-5 py-3 text-right text-gray-200">{fmtTokens(row.total_input_tokens)}</td>
                  <td className="px-5 py-3 text-right text-gray-200">{fmtTokens(row.total_output_tokens)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {(data?.by_model?.length ?? 0) === 0 && !isLoading && (
        <div className="bg-gray-900 rounded-xl border border-gray-800 p-8 text-center text-gray-500 text-sm">
          No usage data in the past 24 hours.
        </div>
      )}
    </div>
  );
}
