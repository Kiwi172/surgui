// @ts-nocheck — this file runs under node --test, which strips types rather
// than checking them; the assertions below cover runtime behavior only.
// The .ts import extension is what node's module resolution requires.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { filterQueue, formatEta, formatSpeed, groupByStatus, progressRatio } from "./queue.ts";
import type { QueueItem } from "./types.ts";

const items: QueueItem[] = [
  { id: "a1", name: "Ubuntu ISO", status: "active", bytesDone: 500, bytesTotal: 1000, speedBps: 100, addedAt: 1 },
  { id: "q1", name: "Debian ISO", status: "queued", bytesDone: 0, bytesTotal: 1000, speedBps: 0, addedAt: 2 },
  { id: "p1", name: "Paused Build", status: "paused", bytesDone: 200, bytesTotal: 1000, speedBps: 0, addedAt: 3 },
  { id: "f1", name: "Broken Archive", status: "failed", bytesDone: 100, bytesTotal: 1000, speedBps: 0, addedAt: 4, error: "checksum mismatch" },
  { id: "c1", name: "Done Tarball", status: "completed", bytesDone: 1000, bytesTotal: 1000, speedBps: 0, addedAt: 5 },
];

describe("groupByStatus", () => {
  it("places one item in each status bucket", () => {
    const groups = groupByStatus(items);
    assert.equal(groups.active.length, 1);
    assert.equal(groups.queued.length, 1);
    assert.equal(groups.paused.length, 1);
    assert.equal(groups.failed.length, 1);
    assert.equal(groups.completed.length, 1);
    assert.equal(groups.active[0].id, "a1");
  });
});

describe("filterQueue", () => {
  it("filters by status", () => {
    assert.equal(filterQueue(items, "", "failed").map((i) => i.id).join(","), "f1");
  });

  it("searches names case-insensitively across all statuses", () => {
    assert.deepEqual(
      filterQueue(items, "iso", "all").map((i) => i.id).sort(),
      ["a1", "q1"],
    );
  });

  it("combines search and status", () => {
    assert.equal(filterQueue(items, "iso", "queued").map((i) => i.id).join(","), "q1");
    assert.deepEqual(filterQueue(items, "no-such-file", "all"), []);
  });
});

describe("formatting", () => {
  it("formats progress ratio 0..1", () => {
    assert.equal(progressRatio(items[0]), 0.5);
  });

  it("formats speed with units", () => {
    assert.equal(formatSpeed(0), "—");
    assert.equal(formatSpeed(1500), "1.5 KB/s");
  });

  it("formats ETA from remaining bytes and speed", () => {
    // 500 bytes left at 100 B/s -> 5 s left
    assert.equal(formatEta(items[0]), "5 s left");
    assert.equal(formatEta(items[4]), "Done");
  });
});

describe("groupByStatus transitional statuses", () => {
  it("buckets pausing and unknown as distinct statuses without throwing", () => {
    const extra = [
      { id: "pg1", name: "Pausing File", status: "pausing", bytesDone: 10, bytesTotal: 100, speedBps: 0, addedAt: 6 },
      { id: "u1", name: "Mystery File", status: "unknown", bytesDone: 0, bytesTotal: 100, speedBps: 0, addedAt: 7 },
    ];
    const groups = groupByStatus(extra as unknown as QueueItem[]);
    assert.ok(groups.pausing.some((i) => i.id === "pg1"), "pausing is distinct from paused");
    assert.ok(groups.unknown.some((i) => i.id === "u1"), "unknown status must be bucketed without throwing");
    assert.equal(groups.paused.length, 0, "paused stays empty when only pausing/unknown present");
  });
});
