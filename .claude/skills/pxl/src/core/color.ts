// Colors are packed 0xRRGGBBAA unsigned integers. 0 is fully transparent.
export type Color = number;

export const TRANSPARENT: Color = 0;

export function pack(r: number, g: number, b: number, a = 255): Color {
  return (((r & 255) << 24) | ((g & 255) << 16) | ((b & 255) << 8) | (a & 255)) >>> 0;
}

export function channels(c: Color): [number, number, number, number] {
  return [(c >>> 24) & 255, (c >>> 16) & 255, (c >>> 8) & 255, c & 255];
}

/** "#rgb", "#rgba", "#rrggbb", "#rrggbbaa" (leading # optional). */
export function parseHex(s: string): Color | undefined {
  let h = s.startsWith("#") ? s.slice(1) : s;
  if (!/^[0-9a-fA-F]+$/.test(h)) return undefined;
  if (h.length === 3 || h.length === 4) h = h.replace(/./g, (d) => d + d);
  if (h.length === 6) h += "ff";
  if (h.length !== 8) return undefined;
  const c = parseInt(h, 16) >>> 0;
  return (c & 255) === 0 ? TRANSPARENT : c;
}

export function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2, d = mx - mn;
  if (!d) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  const h = mx === r ? ((g - b) / d + 6) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s, x = c * (1 - Math.abs(((h / 60) % 2) - 1)), m = l - c / 2;
  const [r, g, b] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

/** Rotate hue a toward b by at most `by` degrees (shortest way round). */
function toward(a: number, b: number, by: number): number {
  const d = ((b - a + 540) % 360) - 180;
  return (a + Math.sign(d) * Math.min(Math.abs(d), by) + 360) % 360;
}

export interface ShadeOpts {
  /** Max hue rotation in degrees at the ends (default 24): shadows cool, highlights warm. */
  shift?: number;
  /** Lightness range around the base, 0..1 (default 0.56). */
  spread?: number;
  /** Saturation multiplier (default 1). */
  sat?: number;
}

/**
 * A hue-shifted ramp from one base color, dark -> light: shadows rotate toward blue-violet and get a
 * little more saturated, highlights rotate toward warm yellow and desaturate, like hand-made palettes.
 */
export function shadeRamp(base: Color, n: number, o: ShadeOpts = {}): Color[] {
  const [r, g, b] = channels(base);
  const [h, s0, l0] = rgbToHsl(r, g, b);
  const shift = o.shift ?? 24, spread = o.spread ?? 0.56, sat = o.sat ?? 1;
  return Array.from({ length: n }, (_, i) => {
    const t = n === 1 ? 0 : i / (n - 1) - 0.5;                  // -0.5 (darkest) .. 0.5 (lightest)
    const l = Math.max(0.04, Math.min(0.96, l0 + t * spread));
    const hh = t < 0 ? toward(h, 250, shift * -t * 2) : toward(h, 55, shift * t * 2);
    const s = Math.max(0, Math.min(1, s0 * sat * (t < 0 ? 1 + 0.15 * -t * 2 : 1 - 0.3 * t * 2)));
    const [rr, gg, bb] = hslToRgb(hh, s, l);
    return pack(rr, gg, bb);
  });
}

export function toHex(c: Color): string {
  const [r, g, b, a] = channels(c);
  const h = (n: number) => n.toString(16).padStart(2, "0");
  return "#" + h(r) + h(g) + h(b) + (a === 255 ? "" : h(a));
}

/** Darken toward black by k (0..1); result is opaque. */
export function darken(c: Color, k: number): Color {
  const [r, g, b] = channels(c);
  return pack(r * (1 - k), g * (1 - k), b * (1 - k));
}
