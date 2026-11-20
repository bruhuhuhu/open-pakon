// The scanner inset: taken off each frame in sensor coordinates before the
// picture is turned upright, and kept in settings.json. Zero trims nothing.
//
// Top is the high sensor column (the top of the Frames strip). Bottom is
// column 0. Leading and trailing run along the film. Percent is of the
// sensor width for the long edges, and of that frame's length for the
// along-film edges. The same rounding and caps as rust/crates/pakon-roll/src/trim.rs.
import { reactive, watch } from 'vue';
import { postJSON } from '../api';
import { app } from './app';

export type TrimUnit = 'px' | 'percent';

export interface EdgeTrim {
  unit: TrimUnit;
  leading: number;
  trailing: number;
  top: number;
  bottom: number;
  auto: boolean;
}

export const trim = reactive<EdgeTrim>({ unit: 'px', leading: 0, trailing: 0, top: 0, bottom: 0, auto: false });

function roundTiesEven(x: number): number {
  const f = Math.floor(x);
  const d = x - f;
  if (d > 0.5) return f + 1;
  if (d < 0.5) return f;
  return f % 2 === 0 ? f : f + 1;
}

function finite(v: unknown): number {
  const n = typeof v === 'number' ? v : Number(v);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

export function normalizeTrim(v: { unit?: string; leading?: unknown; trailing?: unknown; top?: unknown; bottom?: unknown; auto?: unknown } | null | undefined): EdgeTrim {
  const unit: TrimUnit = v?.unit === 'percent' ? 'percent' : 'px';
  const cap = unit === 'percent' ? 25 : 10_000;
  const edge = (n: unknown) => Math.min(cap, finite(n));
  return {
    unit,
    leading: edge(v?.leading),
    trailing: edge(v?.trailing),
    top: edge(v?.top),
    bottom: edge(v?.bottom),
    auto: !!v?.auto,
  };
}

function capPair(a: number, b: number, side: number): [number, number] {
  if (side <= 0) return [0, 0];
  const maxEach = Math.floor(side / 4);
  a = Math.min(Math.max(0, a), maxEach);
  b = Math.min(Math.max(0, b), maxEach);
  const maxSum = side - Math.max(Math.floor(side / 2), 1);
  const sum = a + b;
  if (sum > maxSum && sum > 0) {
    a = Math.floor((a * maxSum) / sum);
    b = Math.floor((b * maxSum) / sum);
  }
  return [a, b];
}

function edgePx(amount: number, side: number): number {
  if (side <= 0 || !(amount > 0)) return 0;
  const px = trim.unit === 'percent' ? roundTiesEven((Math.min(25, amount) / 100) * side) : roundTiesEven(amount);
  return Math.max(0, px);
}

// Scan pixels actually removed, after the quarter-side cap.
export function resolvedTrim(frameLen: number, cols: number) {
  const [leading, trailing] = capPair(edgePx(trim.leading, frameLen), edgePx(trim.trailing, frameLen), frameLen);
  const [bottom, top] = capPair(edgePx(trim.bottom, cols), edgePx(trim.top, cols), cols);
  return { leading, trailing, top, bottom };
}

let hydrated = false;
let dirty = false;
let generation = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

function applyStatus(v: Parameters<typeof normalizeTrim>[0]) {
  if (hydrated || dirty || !v) return;
  hydrated = true;
  Object.assign(trim, normalizeTrim(v));
}

// The status poll carries the copy saved last session. Once the operator
// edits, that copy no longer overwrites what they are typing.
watch(() => app.status?.edge_trim, v => applyStatus(v), { immediate: true });

async function save(gen: number) {
  try {
    const resp = await postJSON('/api/edge-trim', { ...trim });
    if (!resp.ok || gen !== generation) return;
    const saved = await resp.json();
    if (gen !== generation) return;
    Object.assign(trim, normalizeTrim(saved));
    hydrated = true;
  } catch {
    // the boxes already show the typed value; the next change tries again
  }
}

export function scheduleTrimSave() {
  dirty = true;
  generation += 1;
  const gen = generation;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => { void save(gen); }, 250);
}

export function setTrim(next: Partial<EdgeTrim>) {
  Object.assign(trim, normalizeTrim({ ...trim, ...next }));
  scheduleTrimSave();
}
