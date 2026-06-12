import { useEffect, useState } from 'react';

/** Returns a short relative time like "5s ago", "3m", "2h", "4d", or full date if older. */
export function formatRelativeTime(input: string | Date): string {
  const t = typeof input === 'string' ? new Date(input).getTime() : input.getTime();
  const now = Date.now();
  const diff = Math.max(0, now - t);

  const sec = Math.floor(diff / 1000);
  if (sec < 5) return 'now';
  if (sec < 60) return `${sec}s ago`;

  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;

  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour}h ago`;

  const day = Math.floor(hour / 24);
  if (day < 7) return `${day}d ago`;

  // Older than a week — show short date
  const d = new Date(t);
  const sameYear = d.getFullYear() === new Date().getFullYear();
  if (sameYear) {
    return `${d.getMonth() + 1}/${d.getDate()}`;
  }
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
}

/** Returns full timestamp like "2026-06-12 10:23:45" for tooltip. */
export function formatFullTime(input: string | Date): string {
  const d = typeof input === 'string' ? new Date(input) : input;
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** Live-updating relative time with full timestamp on hover. */
export function RelativeTime({ time, className }: { time: string | Date; className?: string }) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const id = setInterval(() => setTick(t => t + 1), 30_000);
    return () => clearInterval(id);
  }, []);

  return (
    <span className={className} title={formatFullTime(time)}>
      {formatRelativeTime(time)}
    </span>
  );
}
