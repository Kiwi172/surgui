import type { DownloadStatus, QueueItem, StatusFilter } from "./types";

export function groupByStatus(items: QueueItem[]): Record<DownloadStatus, QueueItem[]> {
  const groups: Record<DownloadStatus, QueueItem[]> = {
    active: [],
    queued: [],
    pausing: [],
    paused: [],
    failed: [],
    completed: [],
    unknown: [],
  };
  for (const item of items) {
    const bucket = (item.status in groups ? item.status : "unknown") as DownloadStatus;
    groups[bucket].push(item);
  }
  return groups;
}

export function filterQueue(items: QueueItem[], query: string, status: StatusFilter): QueueItem[] {
  const q = query.trim().toLowerCase();
  return items.filter((item) => {
    if (status !== "all" && item.status !== status) return false;
    if (q.length > 0 && !item.name.toLowerCase().includes(q)) return false;
    return true;
  });
}

export function progressRatio(item: QueueItem): number {
  if (item.bytesTotal <= 0) return 0;
  const r = item.bytesDone / item.bytesTotal;
  if (Number.isNaN(r)) return 0;
  return Math.min(1, Math.max(0, r));
}

function formatBytes(n: number): string {
  if (n < 1000) return `${n} B`;
  if (n < 1_000_000) return `${trim(n / 1000)} KB`;
  if (n < 1_000_000_000) return `${trim(n / 1_000_000)} MB`;
  return `${trim(n / 1_000_000_000)} GB`;
}

function trim(n: number): string {
  const r = Math.round(n * 10) / 10;
  return Number.isInteger(r) ? `${r}.0` : `${r}`;
}

export function formatSpeed(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "—";
  return `${formatBytes(Math.round(bps))}/s`;
}

export function formatEta(item: QueueItem): string {
  if (item.status === "completed" || (item.bytesTotal > 0 && item.bytesDone >= item.bytesTotal)) {
    return "Done";
  }
  if (item.status === "failed") return "Failed";
  if (item.status === "paused") return "Paused";
  if (item.status === "pausing") return "Pausing…";
  if (item.status === "queued") return "Waiting";
  if (item.status === "unknown") return "—";
  if (!Number.isFinite(item.speedBps) || item.speedBps <= 0) return "—";
  const remaining = Math.max(0, item.bytesTotal - item.bytesDone);
  const secs = Math.ceil(remaining / item.speedBps);
  if (secs < 60) return `${secs} s left`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} min left`;
  const hours = Math.floor(mins / 60);
  return `${hours} h left`;
}

export function formatSizePair(item: QueueItem): string {
  return `${formatBytes(item.bytesDone)} of ${formatBytes(item.bytesTotal)}`;
}
