<script setup lang="ts">
// The four sensor edges, shared by Frames (where the boxes show it) and
// Settings (where it is explained). Saved as it is edited.
import { reactive, watch } from 'vue';
import { trim, setTrim, type EdgeTrim, type TrimUnit } from '../state/trim';
import InfoTip from '../ui/InfoTip.vue';

const edges = [
  { key: 'top', name: 'Top', title: 'Top of the strip, the sensor edge that lands on top of a landscape frame' },
  { key: 'bottom', name: 'Bottom', title: 'Bottom of the strip, the other long edge of the sensor' },
  { key: 'leading', name: 'Leading', title: 'Left of each box, along the film' },
  { key: 'trailing', name: 'Trailing', title: 'Right of each box, along the film' },
] as const;
type EdgeKey = (typeof edges)[number]['key'];

const tip = 'Taken off the frame in scanner pixels, before the picture is turned upright. '
  + 'Top and bottom are the sensor\'s long edges, across the film: they stay put when you flip the strip. '
  + 'Top is the top of the strip here, the top of a landscape frame, and the right or left side of a portrait frame. '
  + 'At Base 8, 10 pixels (about 0.7%) is the thin white line. 0 changes nothing.';

const shown = (n: number) => (trim.unit === 'px' ? String(Math.round(n)) : String(n));
const draft = reactive<Record<EdgeKey, string>>({ top: '0', bottom: '0', leading: '0', trailing: '0' });

// A half-typed "0." must stay on screen; only a finished number moves the trim.
watch(() => [trim.unit, trim.top, trim.bottom, trim.leading, trim.trailing], () => {
  for (const e of edges) {
    const el = document.activeElement;
    if (el instanceof HTMLInputElement && el.dataset.edge === e.key) continue;
    draft[e.key] = shown(trim[e.key]);
  }
}, { immediate: true });

function onUnit(e: Event) {
  setTrim({ unit: (e.target as HTMLSelectElement).value as TrimUnit });
}
function onEdge(key: EdgeKey, e: Event) {
  const raw = (e.target as HTMLInputElement).value;
  draft[key] = raw;
  if (raw.trim() === '' || raw === '.' || raw === '-' || raw.endsWith('.')) return;
  const n = Number(raw);
  if (Number.isFinite(n)) setTrim({ [key]: n } as Partial<EdgeTrim>);
}
function onBlur(key: EdgeKey) {
  draft[key] = shown(trim[key]);
}
function onAuto(e: Event) {
  setTrim({ auto: (e.target as HTMLInputElement).checked });
}
</script>

<template>
  <div class="flex flex-wrap items-end gap-x-4 gap-y-2.5">
    <div class="flex items-center gap-2 pb-1.5 font-mono text-small font-semibold uppercase">
      Sensor trim
      <InfoTip label="About the sensor trim" :tip="tip" />
    </div>
    <label class="flex items-center gap-2 font-mono text-small font-semibold uppercase">Unit
      <select :value="trim.unit" aria-label="Trim unit" @change="onUnit">
        <option value="px">px</option>
        <option value="percent">%</option>
      </select>
    </label>
    <label v-for="e in edges" :key="e.key" class="flex items-center gap-1.5 font-mono text-small font-semibold uppercase" :title="e.title">
      {{ e.name }}
      <input type="number" inputmode="decimal" min="0" :max="trim.unit === 'percent' ? 25 : 10000"
             :step="trim.unit === 'percent' ? 0.1 : 1" class="w-16"
             :data-edge="e.key"
             :aria-label="'Trim the ' + e.name.toLowerCase() + ' sensor edge'"
             :value="draft[e.key]" @input="onEdge(e.key, $event)" @blur="onBlur(e.key)">
    </label>
    <label class="flex items-center gap-2 pb-1 font-mono text-small font-semibold uppercase" title="Drop a blank white band, at most 2% of each edge">
      <input type="checkbox" :checked="trim.auto" @change="onAuto">
      Auto blank edge
    </label>
    <InfoTip label="About auto-trim" place="left"
             tip="After the fixed trim, drop a near-uniform white band on any edge, at most 2% of that side. The fixed numbers above are what matches a scanner that always overruns the same edge." />
  </div>
</template>
