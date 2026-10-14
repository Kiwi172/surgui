export type DownloadStatus = "active" | "queued" | "pausing" | "paused" | "failed" | "completed" | "unknown";

export interface QueueItem {
  id: string;
  name: string;
  status: DownloadStatus;
  bytesDone: number;
  bytesTotal: number;
  /** bytes per second; 0 when stalled or not started */
  speedBps: number;
  addedAt: number;
  error?: string;
}

export type StatusFilter = "all" | DownloadStatus;

export const STATUS_ORDER: DownloadStatus[] = ["active", "queued", "pausing", "paused", "failed", "completed", "unknown"];
