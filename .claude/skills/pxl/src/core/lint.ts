// Pixel-art critic: finds the classic mistakes in a built image and says where they are, so an agent
// (or a person) can fix exact pixels instead of guessing from a preview.
import { type Color, TRANSPARENT, channels, rgbToHsl, toHex } from "./color.ts";
import type { Img } from "./image.ts";

export interface Finding {
  rule: "orphan" | "jaggy" | "pillow" | "near-duplicate" | "colors";
  message: string;
  /** Pixels involved (x, y), when the finding is local. */
  at: [number, number][];
}

const lum = (c: Color) => { const [r, g, b] = channels(c); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };

export function lint(img: Img, name = "asset"): Finding[] {
  const { w, h } = img, out: Finding[] = [];
  const get = (x: number, y: number) => (x < 0 || y < 0 || x >= w || y >= h ? TRANSPARENT : img.get(x, y));
  const N8 = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]];

  // Orphans: an opaque pixel with no same-colored neighbor, inside a shape (all 4 sides opaque).
  // Single pixels on purpose (eyes, glints) usually sit in high contrast; noise sits in low contrast.
  const orphans: [number, number][] = [];
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const c = get(x, y);
      if (c === TRANSPARENT) continue;
      if (N8.some(([dx, dy]) => get(x + dx, y + dy) === c)) continue;
      if ([get(x - 1, y), get(x + 1, y), get(x, y - 1), get(x, y + 1)].some((n) => n === TRANSPARENT)) continue;
      const around = N8.map(([dx, dy]) => lum(get(x + dx, y + dy))).reduce((a, v) => a + v, 0) / 8;
      if (Math.abs(lum(c) - around) < 48) orphans.push([x, y]);
    }
  if (orphans.length)
    out.push({ rule: "orphan", at: orphans, message: `${orphans.length} stray pixel${orphans.length > 1 ? "s" : ""} with no same-color neighbor and little contrast: noise rather than detail. Merge them into a neighbor color or group them into a shape.` });

  // Jaggies: in a 1-px line, an L-shaped corner (one horizontal + one vertical same-color neighbor,
  // the diagonal between them empty of that color) "doubles" the line and makes it look ragged.
  // Only lines count: 8-connected same-color strokes of 6+ pixels without a 2x2 block. Small
  // clusters (pupils, glints, shading marks) are shapes, and their L corners are fine.
  const comp = new Int32Array(w * h).fill(-1), compSize: number[] = [], compThin: boolean[] = [];
  for (let y0 = 0; y0 < h; y0++) for (let x0 = 0; x0 < w; x0++) {
    const c = get(x0, y0);
    if (c === TRANSPARENT || comp[y0 * w + x0] >= 0) continue;
    const id = compSize.length, stack = [y0 * w + x0];
    comp[stack[0]] = id;
    let size = 0, thin = true;
    while (stack.length) {
      const i = stack.pop()!, x = i % w, y = (i / w) | 0;
      size++;
      if (get(x + 1, y) === c && get(x, y + 1) === c && get(x + 1, y + 1) === c) thin = false;
      for (const [dx, dy] of N8) {
        const nx = x + dx, ny = y + dy;
        if (get(nx, ny) !== c || comp[ny * w + nx] >= 0) continue;
        comp[ny * w + nx] = id; stack.push(ny * w + nx);
      }
    }
    compSize.push(size); compThin.push(thin);
  }
  const jag: [number, number][] = [];
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const c = get(x, y);
      if (c === TRANSPARENT) continue;
      const same = (dx: number, dy: number) => get(x + dx, y + dy) === c;
      const count = N8.filter(([dx, dy]) => same(dx, dy)).length;
      const id = comp[y * w + x];
      if (count !== 2 || compSize[id] < 6 || !compThin[id]) continue;
      // A curve turning a corner keeps going straight on both arms (that's fine); a doubled pixel
      // sits inside a diagonal, so neither arm continues straight.
      for (const [hx, vy] of [[1, 1], [1, -1], [-1, 1], [-1, -1]])
        if (same(hx, 0) && same(0, vy) && !same(hx, vy) && !same(2 * hx, 0) && !same(0, 2 * vy)) { jag.push([x, y]); break; }
    }
  if (jag.length)
    out.push({ rule: "jaggy", at: jag, message: `${jag.length} doubled corner${jag.length > 1 ? "s" : ""} in 1-px lines (an L where the line should step diagonally). Remove the corner pixel so the line steps cleanly.` });

  // Pillow shading: brightness that follows the outline (light in the middle, dark all around)
  // instead of coming from a direction. Measured per color family (a cap and its stem are judged
  // apart), with distance taken from that family's own edge.
  const family = (c: Color) => { const [r, g, b] = channels(c); const [hu, sa, li] = rgbToHsl(r, g, b); return sa < 0.15 || li < 0.1 || li > 0.88 ? -1 : Math.round(hu / 40) % 9; };
  const fam = new Int8Array(w * h).fill(-2);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const c = get(x, y); if (c !== TRANSPARENT) fam[y * w + x] = family(c); }
  const famAt = (x: number, y: number) => (x < 0 || y < 0 || x >= w || y >= h ? -2 : fam[y * w + x]);
  const pillows: number[] = [];
  for (const f of new Set(fam)) {
    if (f < 0) continue;
    const dist = new Int32Array(w * h).fill(-1), q: number[] = [];
    // neutrals (spots, glints, gray) inside the region don't end it; transparency and other hues do
    const inside = (x: number, y: number) => { const g = famAt(x, y); return g === f || g === -1; };
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      if (!inside(x, y)) continue;
      if (N8.some(([dx, dy]) => !inside(x + dx, y + dy))) { dist[y * w + x] = 0; q.push(y * w + x); }
    }
    for (let k = 0; k < q.length; k++) {
      const i = q[k], x = i % w, y = (i / w) | 0;
      for (const [dx, dy] of N8) {
        const nx = x + dx, ny = y + dy, j = ny * w + nx;
        if (!inside(nx, ny) || dist[j] >= 0) continue;
        dist[j] = dist[i] + 1; q.push(j);
      }
    }
    let n = 0, sd = 0, sl = 0, sdd = 0, sll = 0, sdl = 0, cx = 0, cy = 0, lx = 0, ly = 0, lw = 0;
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const d = dist[y * w + x];
      if (d < 1 || fam[y * w + x] !== f) continue;     // skip the rim itself and the neutrals
      const l = lum(get(x, y));
      n++; sd += d; sl += l; sdd += d * d; sll += l * l; sdl += d * l;
      cx += x; cy += y; lx += x * l; ly += y * l; lw += l;
    }
    if (n < 40) continue;
    const corr = (n * sdl - sd * sl) / Math.sqrt(Math.max(1e-9, (n * sdd - sd * sd) * (n * sll - sl * sl)));
    const offset = Math.hypot(lx / lw - cx / n, ly / lw - cy / n) / Math.sqrt(n);   // how far the light's center sits from the region's
    if (corr > 0.55 && offset < 0.04) pillows.push(corr);
  }
  if (pillows.length)
    out.push({ rule: "pillow", at: [], message: `pillow shading: brightness rises toward the middle on every side (correlation ${Math.max(...pillows).toFixed(2)} with distance from the edge, light centered). Pick a light direction and shade the far side darker.` });

  // Colors: too many, or pairs so close they read as one.
  const counts = new Map<Color, number>();
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const c = get(x, y); if (c !== TRANSPARENT) counts.set(c, (counts.get(c) ?? 0) + 1); }
  const cols = [...counts.keys()];
  if (cols.length > 24 && w * h <= 64 * 64)
    out.push({ rule: "colors", at: [], message: `${cols.length} colors in a ${w}x${h} ${name}: small sprites read best with a tight palette (8-16). Merge similar shades.` });
  // Past 64 colors the image is a lit or blended render, not a hand palette: pairs don't apply.
  const pairs: string[] = [];
  if (cols.length <= 64)
    for (let i = 0; i < cols.length; i++)
      for (let j = i + 1; j < cols.length; j++) {
        const [a, b] = [channels(cols[i]), channels(cols[j])];
        const d = Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        if (d > 0 && d < 10) pairs.push(`${toHex(cols[i])}~${toHex(cols[j])}`);
      }
  if (pairs.length)
    out.push({ rule: "near-duplicate", at: [], message: `${pairs.length} color pair${pairs.length > 1 ? "s" : ""} so close they read as one (${pairs.slice(0, 6).join(" ")}${pairs.length > 6 ? " …" : ""}): merge them or push them apart.` });
  return out;
}
