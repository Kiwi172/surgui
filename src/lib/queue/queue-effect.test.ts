// @ts-nocheck — exercised at runtime by node --test (type stripping).
// Regression guard for the QueueScreen.svelte items-sync $effect.
// An effect that both reads and writes rows/selectedId retriggers itself:
// every write schedules another run and the main thread never settles.
// A full component mount is out of scope here (node --test cannot import
// .svelte), so this test uses Svelte's reactivity core
// (svelte/internal/client) plus a source guard on the actual component.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { effect_root, user_effect, state, get, set, untrack } from "svelte/internal/client";

const here = path.dirname(fileURLToPath(import.meta.url));
const screenPath = path.join(here, "..", "QueueScreen.svelte");

const SAMPLE = [
  { id: "dl-101", name: "surge-macos-aarch64.tar.gz" },
  { id: "dl-102", name: "edge-firmware-bundle-2410.bin" },
];

function flush(ms = 60) {
  return new Promise((r) => setTimeout(r, ms));
}

describe("QueueScreen effect loop regression", () => {
  it("reproduces the runaway loop with the old read+write effect shape", async () => {
    // Effect shape under test: reads rows/selectedId and writes them.
    const items = state(SAMPLE);
    const rows = state([]);
    const selectedId = state(null);
    let runs = 0;
    const cleanup = effect_root(() => {
      user_effect(() => {
        runs++;
        if (runs > 20) return; // guard so the test itself terminates
        set(rows, get(items).map((i) => ({ ...i })));
        const r = get(rows);
        const s = get(selectedId);
        if (r.length > 0 && (s === null || !r.some((x) => x.id === s))) set(selectedId, r[0].id);
        if (r.length === 0) set(selectedId, null);
      });
    });
    await flush(120);
    cleanup();
    // Without a guard the effect never settles: every write retriggers it
    // because rows/selectedId were read inside the same effect.
    assert.ok(runs > 5, `expected runaway effect (runs > 5), got ${runs}`);
  });

  it("QueueScreen sync effect does not self-trigger (uses untrack)", () => {
    const src = fs.readFileSync(screenPath, "utf8");
    const effectBlocks = [...src.matchAll(/\$effect\s*\(\s*\(\)\s*=>\s*\{([\s\S]*?)\n\s*\}\);/g)].map((m) => m[1]);
    assert.ok(effectBlocks.length > 0, "expected at least one $effect in QueueScreen.svelte");
    for (const body of effectBlocks) {
      const writesRows = /(^|[^.\w])rows\s*=/.test(body) || /(^|[^.\w])selectedId\s*=/.test(body);
      if (writesRows) {
        assert.ok(
          body.includes("untrack"),
          "effect writes rows/selectedId so it must wrap writes in untrack() to avoid self-triggering",
        );
      }
    }
    assert.ok(
      /import\s*\{[^}]*untrack[^}]*\}\s*from\s*["']svelte["']/.test(src),
      'expected `import { untrack } from "svelte"` in QueueScreen.svelte',
    );
  });

  it("untracked sync settles in one run and renders sample rows", async () => {
    // Component logic: track items only, write rows/selection untracked.
    const items = state(SAMPLE);
    const rows = state([]);
    const selectedId = state(null);
    let runs = 0;
    const cleanup = effect_root(() => {
      user_effect(() => {
        runs++;
        const next = get(items).map((i) => ({ ...i }));
        untrack(() => {
          set(rows, next);
          const s = get(selectedId);
          if (next.length > 0 && (s === null || !next.some((x) => x.id === s))) set(selectedId, next[0].id);
          if (next.length === 0) set(selectedId, null);
        });
      });
    });
    await flush(120);
    assert.equal(runs, 1, `expected sync effect to settle after 1 run, got ${runs}`);
    assert.equal(get(rows).length, 2, "expected 2 sample rows rendered");
    assert.equal(get(selectedId), "dl-101", "expected first sample row selected");
    // Local row actions (pause/remove) must not retrigger the items-sync effect.
    set(rows, get(rows).filter((r) => r.id !== "dl-101"));
    await flush(80);
    assert.equal(runs, 1, `local row edit must not retrigger sync (runs=${runs})`);
    cleanup();
  });
});
