import type { QueueItem } from "./queue/types.ts";

/**
 * SAMPLE DATA — browser preview only.
 * Rendered solely when the Tauri runtime is absent. Never persisted,
 * never fetched, contains no Surge token and no network addresses.
 */
export const SAMPLE_ITEMS: QueueItem[] = [
  {
    id: "dl-101",
    name: "surge-macos-aarch64.tar.gz",
    status: "active",
    bytesDone: 84_400_000,
    bytesTotal: 212_000_000,
    speedBps: 4_800_000,
    addedAt: 1729000001,
  },
  {
    id: "dl-102",
    name: "edge-firmware-bundle-2410.bin",
    status: "active",
    bytesDone: 12_000_000,
    bytesTotal: 96_000_000,
    speedBps: 1_100_000,
    addedAt: 1729000002,
  },
  {
    id: "dl-103",
    name: "nightly-symbols.zip",
    status: "queued",
    bytesDone: 0,
    bytesTotal: 44_000_000,
    speedBps: 0,
    addedAt: 1729000003,
  },
  {
    id: "dl-104",
    name: "region-map-tiles.pack",
    status: "paused",
    bytesDone: 30_500_000,
    bytesTotal: 61_000_000,
    speedBps: 0,
    addedAt: 1729000004,
  },
  {
    id: "dl-105",
    name: "diagnostics-2026-10-11.log",
    status: "failed",
    bytesDone: 2_100_000,
    bytesTotal: 9_400_000,
    speedBps: 0,
    addedAt: 1729000005,
    error: "Checksum mismatch on chunk 14. Retry resumes from the last good offset.",
  },
  {
    id: "dl-106",
    name: "surge-linux-x86_64.tar.gz",
    status: "completed",
    bytesDone: 198_000_000,
    bytesTotal: 198_000_000,
    speedBps: 0,
    addedAt: 1729000006,
  },
];
