// Raster images into palettes: box-filter resampling, perceptual nearest-color matching with an
// optional ordered dither, and palette extraction (k-means in OKLab). Shared by the image() op
// and `pxl trace`, so a photo, a render or any PNG can become pixel art in the file's own palette.
import { type Color, TRANSPARENT, channels, pack } from "./color.ts";
import { PxError } from "./errors.ts";
import { bayer, rng } from "./util.ts";

/** Straight (not premultiplied) 8-bit RGBA. */
export interface Rgba {
  w: number;
  h: number;
  rgba: Uint8Array;
}

/** sRGB 0..255 -> OKLab, so color distances match what eyes see. */
export function oklab(r: number, g: number, b: number): [number, number, number] {
  const lin = (c: number) => { c /= 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  const R = lin(r), G = lin(g), B = lin(b);
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}

/** Resize to w×h: area average (alpha-weighted) when shrinking, nearest neighbour when growing. */
export function resample(src: Rgba, w: number, h: number): Rgba {
  w = Math.max(1, Math.round(w)); h = Math.max(1, Math.round(h));
  if (w === src.w && h === src.h) return src;
  const out = new Uint8Array(w * h * 4), s = src.rgba;
  if (w >= src.w && h >= src.h) {
    for (let y = 0; y < h; y++) {
      const sy = Math.min(src.h - 1, Math.floor(((y + 0.5) * src.h) / h));
      for (let x = 0; x < w; x++) {
        const sx = Math.min(src.w - 1, Math.floor(((x + 0.5) * src.w) / w));
        out.set(s.subarray((sy * src.w + sx) * 4, (sy * src.w + sx) * 4 + 4), (y * w + x) * 4);
      }
    }
    return { w, h, rgba: out };
  }
  for (let y = 0; y < h; y++) {
    const y0 = (y * src.h) / h, y1 = ((y + 1) * src.h) / h;
    for (let x = 0; x < w; x++) {
      const x0 = (x * src.w) / w, x1 = ((x + 1) * src.w) / w;
      let r = 0, g = 0, b = 0, a = 0, area = 0;
      for (let sy = Math.floor(y0); sy < Math.min(src.h, Math.ceil(y1)); sy++) {
        const wy = Math.min(y1, sy + 1) - Math.max(y0, sy);
        for (let sx = Math.floor(x0); sx < Math.min(src.w, Math.ceil(x1)); sx++) {
          const wgt = (Math.min(x1, sx + 1) - Math.max(x0, sx)) * wy, i = (sy * src.w + sx) * 4, al = (s[i + 3] / 255) * wgt;
          r += s[i] * al; g += s[i + 1] * al; b += s[i + 2] * al; a += al; area += wgt;
        }
      }
      const o = (y * w + x) * 4;
      if (a > 0) { out[o] = Math.round(r / a); out[o + 1] = Math.round(g / a); out[o + 2] = Math.round(b / a); }
      out[o + 3] = area > 0 ? Math.round((255 * a) / area) : 0;
    }
  }
  return { w, h, rgba: out };
}

export interface QuantOpts {
  /** Ordered (Bayer) dither strength 0..1: 0 = hard nearest color (default), 1 = full checker blending. */
  dither?: number;
  /** Pixels with alpha below this are transparent (default 128). */
  alpha?: number;
}

/** Each pixel becomes the nearest of `colors` (in OKLab), or TRANSPARENT below the alpha threshold. */
export function quantize(src: Rgba, colors: Color[], o: QuantOpts = {}): Uint32Array {
  const pal = [...new Set(colors.filter((c) => c !== TRANSPARENT && (c & 255) !== 0))];
  if (!pal.length) throw new PxError("quantize: the palette has no visible colors");
  const labs = pal.map((c) => { const [r, g, b] = channels(c); return oklab(r, g, b); });
  const thr = o.alpha ?? 128, dither = Math.max(0, Math.min(1, o.dither ?? 0));
  const out = new Uint32Array(src.w * src.h), cache = new Map<number, Color>();
  const clamp = (v: number) => (v < 0 ? 0 : v > 255 ? 255 : Math.round(v));
  for (let y = 0; y < src.h; y++)
    for (let x = 0; x < src.w; x++) {
      const i = (y * src.w + x) * 4;
      if (src.rgba[i + 3] < thr) continue;
      let r = src.rgba[i], g = src.rgba[i + 1], b = src.rgba[i + 2];
      if (dither > 0) {
        const d = (bayer(x, y) - 0.5) * dither * 72;
        r = clamp(r + d); g = clamp(g + d); b = clamp(b + d);
      }
      const key = (r << 16) | (g << 8) | b;
      let c = cache.get(key);
      if (c === undefined) {
        const [L, A, B] = oklab(r, g, b);
        let best = 0, bd = Infinity;
        for (let k = 0; k < labs.length; k++) {
          const d = (labs[k][0] - L) ** 2 + (labs[k][1] - A) ** 2 + (labs[k][2] - B) ** 2;
          if (d < bd) { bd = d; best = k; }
        }
        c = pal[best];
        cache.set(key, c);
      }
      out[y * src.w + x] = c;
    }
  return out;
}

/**
 * The k most representative opaque colors of an image (k-means++ in OKLab, fixed seed), dark to
 * light. Fewer come back when the image has fewer distinct colors.
 */
export function extractPalette(src: Rgba, k: number, alpha = 128): Color[] {
  const pts: [number, number, number][] = [], rgbs: number[][] = [];
  const total = src.w * src.h, stride = Math.max(1, Math.floor(total / 24000));
  for (let i = 0; i < total; i += stride) {
    const o = i * 4;
    if (src.rgba[o + 3] < alpha) continue;
    pts.push(oklab(src.rgba[o], src.rgba[o + 1], src.rgba[o + 2]));
    rgbs.push([src.rgba[o], src.rgba[o + 1], src.rgba[o + 2]]);
  }
  if (!pts.length) throw new PxError("the image has no opaque pixels");
  const distinct = new Set(rgbs.map((c) => (c[0] << 16) | (c[1] << 8) | c[2]));
  k = Math.max(1, Math.min(Math.round(k), distinct.size));
  const d2 = (a: number[], b: number[]) => (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
  const r = rng(7);
  const cent: number[][] = [pts[Math.floor(r() * pts.length)].slice()];
  const near = pts.map((p) => d2(p, cent[0]));
  while (cent.length < k) {
    let sum = 0;
    for (const v of near) sum += v;
    if (sum === 0) break;
    let t = r() * sum, j = 0;
    while (j < near.length - 1 && (t -= near[j]) > 0) j++;
    cent.push(pts[j].slice());
    for (let i = 0; i < pts.length; i++) near[i] = Math.min(near[i], d2(pts[i], pts[j]));
  }
  const asg = new Int32Array(pts.length);
  for (let it = 0; it < 20; it++) {
    let moved = 0;
    for (let i = 0; i < pts.length; i++) {
      let best = 0, bd = Infinity;
      for (let c = 0; c < cent.length; c++) { const d = d2(pts[i], cent[c]); if (d < bd) { bd = d; best = c; } }
      if (asg[i] !== best || it === 0) { asg[i] = best; moved++; }
    }
    const acc = cent.map(() => [0, 0, 0, 0]);
    for (let i = 0; i < pts.length; i++) { const a = acc[asg[i]]; a[0] += pts[i][0]; a[1] += pts[i][1]; a[2] += pts[i][2]; a[3]++; }
    acc.forEach((a, c) => { if (a[3]) cent[c] = [a[0] / a[3], a[1] / a[3], a[2] / a[3]]; });
    if (!moved) break;
  }
  // Each cluster's color is the mean of its pixels in sRGB: exact source tones, no round trip.
  const sums = cent.map(() => [0, 0, 0, 0]);
  rgbs.forEach((c, i) => { const s = sums[asg[i]]; s[0] += c[0]; s[1] += c[1]; s[2] += c[2]; s[3]++; });
  return sums.filter((s) => s[3] > 0)
    .map((s) => ({ c: pack(Math.round(s[0] / s[3]), Math.round(s[1] / s[3]), Math.round(s[2] / s[3])), L: oklab(s[0] / s[3], s[1] / s[3], s[2] / s[3])[0] }))
    .sort((a, b) => a.L - b.L)
    .map((x) => x.c);
}

/** Mean per-channel error between quantized pixels and the image they came from, over opaque pixels (0 = exact). */
export function fidelity(src: Rgba, data: Uint32Array, alpha = 128): { meanError: number; close: number } {
  let sum = 0, close = 0, n = 0;
  for (let i = 0; i < data.length; i++) {
    const o = i * 4;
    if (src.rgba[o + 3] < alpha) continue;
    const [r, g, b] = channels(data[i]);
    const dr = Math.abs(r - src.rgba[o]), dg = Math.abs(g - src.rgba[o + 1]), db = Math.abs(b - src.rgba[o + 2]);
    sum += (dr + dg + db) / 3;
    if (Math.max(dr, dg, db) <= 16) close++;
    n++;
  }
  return { meanError: n ? Math.round((sum / n) * 10) / 10 : 0, close: n ? close / n : 1 };
}
