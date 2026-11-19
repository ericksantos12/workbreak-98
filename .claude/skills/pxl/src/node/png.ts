import { crc32, deflateSync, inflateSync } from "node:zlib";
import { TRANSPARENT } from "../core/color.ts";
import { PxError } from "../core/errors.ts";
import type { Img } from "../core/image.ts";
import type { Rgba } from "../core/quant.ts";

function chunk(type: string, data: Buffer): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
  return Buffer.concat([head, data, crc]);
}

/** PNG bytes: indexed (with tRNS) when the image has <= 256 colors, RGBA otherwise. */
export function encodePng(img: Img): Buffer {
  const { w, h, data } = img;
  const index = new Map<number, number>();
  for (let i = 0; i < data.length && index.size <= 256; i++) if (!index.has(data[i])) index.set(data[i], index.size);
  const indexed = index.size <= 256;

  const rowLen = indexed ? w + 1 : w * 4 + 1;
  const raw = Buffer.alloc(rowLen * h);
  for (let y = 0; y < h; y++) {
    let o = y * rowLen + 1; // filter byte 0
    for (let x = 0; x < w; x++) {
      const c = data[y * w + x];
      if (indexed) raw[o++] = index.get(c)!;
      else { raw[o++] = c >>> 24; raw[o++] = (c >>> 16) & 255; raw[o++] = (c >>> 8) & 255; raw[o++] = c & 255; }
    }
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = indexed ? 3 : 6;
  const parts = [Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr)];
  if (indexed) {
    const colors = [...index.keys()];
    const plte = Buffer.alloc(colors.length * 3), trns = Buffer.alloc(colors.length);
    let lastAlpha = -1;
    colors.forEach((c, i) => {
      plte[i * 3] = c >>> 24; plte[i * 3 + 1] = (c >>> 16) & 255; plte[i * 3 + 2] = (c >>> 8) & 255;
      trns[i] = c === TRANSPARENT ? 0 : c & 255;
      if (trns[i] !== 255) lastAlpha = i;
    });
    parts.push(chunk("PLTE", plte));
    if (lastAlpha >= 0) parts.push(chunk("tRNS", trns.subarray(0, lastAlpha + 1)));
  }
  parts.push(chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0)));
  return Buffer.concat(parts);
}

/** PNG bytes -> straight 8-bit RGBA: gray, RGB, palette (+ tRNS) and alpha types, 1..16-bit depths, no interlace. */
export function decodePng(buf: Uint8Array): Rgba {
  const v = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  if (buf.length < 8 || v.getUint32(0) !== 0x89504e47) throw new PxError("not a PNG file (pxl reads PNG images)");
  let o = 8, w = 0, h = 0, depth = 0, type = 0, interlace = 0, plte: Uint8Array | undefined, trns: Uint8Array | undefined;
  const idat: Uint8Array[] = [];
  while (o + 8 <= buf.length) {
    const len = v.getUint32(o), kind = String.fromCharCode(buf[o + 4], buf[o + 5], buf[o + 6], buf[o + 7]);
    const data = buf.subarray(o + 8, o + 8 + len);
    if (kind === "IHDR") { w = v.getUint32(o + 8); h = v.getUint32(o + 12); depth = buf[o + 16]; type = buf[o + 17]; interlace = buf[o + 20]; }
    else if (kind === "PLTE") plte = data;
    else if (kind === "tRNS") trns = data;
    else if (kind === "IDAT") idat.push(data);
    else if (kind === "IEND") break;
    o += 12 + len;
  }
  if (!w || !h) throw new PxError("PNG has no IHDR");
  if (interlace) throw new PxError("interlaced PNGs aren't supported: re-save the image without interlacing");
  const chans = ({ 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 } as Record<number, number>)[type];
  if (!chans || ![1, 2, 4, 8, 16].includes(depth)) throw new PxError(`unsupported PNG (color type ${type}, depth ${depth})`);
  const bits = chans * depth, bpp = Math.max(1, Math.ceil(bits / 8)), row = Math.ceil((w * bits) / 8);
  const raw = inflateSync(Buffer.concat(idat.map((d) => Buffer.from(d.buffer, d.byteOffset, d.byteLength))));
  if (raw.length < h * (row + 1)) throw new PxError("PNG data is truncated");
  const px = new Uint8Array(h * row);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (row + 1)], src = raw.subarray(y * (row + 1) + 1, (y + 1) * (row + 1));
    const cur = px.subarray(y * row, (y + 1) * row), up = y ? px.subarray((y - 1) * row, y * row) : undefined;
    for (let i = 0; i < row; i++) {
      const a = i >= bpp ? cur[i - bpp] : 0, b = up ? up[i] : 0, c = up && i >= bpp ? up[i - bpp] : 0;
      let pred = 0;
      if (f === 1) pred = a; else if (f === 2) pred = b; else if (f === 3) pred = (a + b) >> 1;
      else if (f === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      cur[i] = (src[i] + pred) & 255;
    }
  }
  const max = (1 << depth) - 1;
  /** Sample k of pixel x on a row, as 0..255 (16-bit keeps the high byte; low depths are scaled up). */
  const sample = (line: Uint8Array, x: number, k: number): number => {
    const s = x * chans + k;
    if (depth === 8) return line[s];
    if (depth === 16) return line[s * 2];
    const bit = s * depth, byte = line[bit >> 3];
    return (byte >> (8 - depth - (bit & 7))) & max;
  };
  const scale = depth < 8 ? 255 / max : 1;
  const out = new Uint8Array(w * h * 4);
  const tr16 = (k: number) => (trns && trns.length >= (k + 1) * 2 ? (trns[k * 2] << 8) | trns[k * 2 + 1] : -1);
  const raw16 = (line: Uint8Array, x: number, k: number): number => {
    const s = x * chans + k;
    if (depth === 16) return (line[s * 2] << 8) | line[s * 2 + 1];
    return sample(line, x, k);
  };
  for (let y = 0; y < h; y++) {
    const line = px.subarray(y * row, (y + 1) * row);
    for (let x = 0; x < w; x++) {
      const d = (y * w + x) * 4;
      let r: number, g: number, b: number, a = 255;
      if (type === 3) {
        const i = sample(line, x, 0);
        if (!plte || i * 3 + 2 >= plte.length) throw new PxError("PNG palette index out of range");
        r = plte[i * 3]; g = plte[i * 3 + 1]; b = plte[i * 3 + 2];
        if (trns && i < trns.length) a = trns[i];
      } else if (type === 0 || type === 4) {
        r = g = b = Math.round(sample(line, x, 0) * scale);
        if (type === 4) a = Math.round(sample(line, x, 1) * scale);
        else if (trns && raw16(line, x, 0) === tr16(0)) a = 0;
      } else {
        r = Math.round(sample(line, x, 0) * scale); g = Math.round(sample(line, x, 1) * scale); b = Math.round(sample(line, x, 2) * scale);
        if (type === 6) a = Math.round(sample(line, x, 3) * scale);
        else if (trns && raw16(line, x, 0) === tr16(0) && raw16(line, x, 1) === tr16(1) && raw16(line, x, 2) === tr16(2)) a = 0;
      }
      out[d] = r; out[d + 1] = g; out[d + 2] = b; out[d + 3] = a;
    }
  }
  return { w, h, rgba: out };
}
