// `pxl trace`: a PNG (photo, render, reference) into pxl source: a palette (extracted, or a
// built-in one) and a grid string, rebuilt and compared with the image so every trace reports
// its own fidelity. The output is an ordinary art file: patch pixels, recolor keys, stamp it.
import { readFileSync } from "node:fs";
import { TRANSPARENT, toHex } from "../core/color.ts";
import { PxError } from "../core/errors.ts";
import { BUILTIN, Palette } from "../core/palette.ts";
import { type Rgba, extractPalette, fidelity, quantize, resample } from "../core/quant.ts";
import { loadSource } from "./load.ts";
import { decodePng } from "./png.ts";

export interface TraceOpts {
  w?: number;
  h?: number;
  /** Size as a fraction of the image. */
  scale?: number;
  /** Palette size to extract (default 16), unless `palette` names a built-in one. */
  colors?: number;
  palette?: string;
  dither?: number;
  alpha?: number;
  name?: string;
  from?: string;
}

export interface Traced {
  w: number;
  h: number;
  colors: number;
  source: string;
  fit: { meanError: number; close: number };
}

export function traceRgba(img: Rgba, o: TraceOpts = {}): Traced {
  let { w, h } = o;
  if (o.scale !== undefined) {
    if (!(o.scale > 0)) throw new PxError(`--scale must be > 0, got ${o.scale}`);
    w = img.w * o.scale; h = img.h * o.scale;
  } else if (w === undefined && h === undefined) { w = img.w; h = img.h; }
  else if (w === undefined) w = (h! * img.w) / img.h;
  else if (h === undefined) h = (w * img.h) / img.w;
  const small = resample(img, w!, h!);
  let pal: Palette, use: string;
  if (o.palette) {
    if (!BUILTIN[o.palette]) throw new PxError(`unknown palette '${o.palette}' (have: ${Object.keys(BUILTIN).join(", ")})`);
    pal = Palette.builtin(o.palette);
    use = `use(${JSON.stringify(o.palette)})`;
  } else {
    const cols = extractPalette(small, o.colors ?? 16, o.alpha);
    pal = Palette.fromColors(cols);
    use = `use([${cols.map((c) => JSON.stringify(toHex(c))).join(", ")}])`;
  }
  const data = quantize(small, [...pal.list], { dither: o.dither, alpha: o.alpha });
  const rev = pal.reverse(), rows: string[] = [];
  for (let y = 0; y < small.h; y++) {
    let row = "";
    for (let x = 0; x < small.w; x++) { const c = data[y * small.w + x]; row += c === TRANSPARENT ? "." : rev.get(c) ?? "?"; }
    rows.push("  " + row);
  }
  const name = o.name ?? "traced";
  const source = [
    `// Traced${o.from ? ` from ${o.from}` : ""} by \`pxl trace\`: ${small.w}x${small.h}, ${pal.list.length} colors${o.dither ? `, dither ${o.dither}` : ""}.`,
    `// Keys are palette indexes (0-9a-z...); "." is transparent. Patch pixels with grid(), recolor with swap.`,
    use,
    ``,
    `sprite(${JSON.stringify(name)}, \``,
    ...rows,
    `\`)`,
    ``,
  ].join("\n");
  const fit = fidelity(small, data, o.alpha);
  // Prove the source rebuilds the same pixels (the grid parser must round-trip every key).
  const back = loadSource(source, "traced.px.js").image(name);
  if (back.w !== small.w || back.h !== small.h) throw new PxError(`trace: rebuilt ${back.w}x${back.h}, expected ${small.w}x${small.h}`);
  for (let i = 0; i < data.length; i++) if (back.data[i] !== data[i]) throw new PxError(`trace: the rebuilt source differs at pixel ${i % small.w},${Math.floor(i / small.w)}`);
  return { w: small.w, h: small.h, colors: pal.list.length, source, fit };
}

export function traceFile(file: string, o: TraceOpts = {}): Traced {
  let bytes: Buffer;
  try { bytes = readFileSync(file); } catch { throw new PxError(`cannot read image '${file}'`); }
  return traceRgba(decodePng(bytes), { from: file.replace(/^.*[\\/]/, ""), ...o });
}
