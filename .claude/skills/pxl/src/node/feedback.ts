// Feedback from the live viewer: marks painted over an asset, numbered notes and a comment, written
// next to the build outputs so an agent can read exactly what the person pointed at.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { type Color, TRANSPARENT, toHex } from "../core/color.ts";
import { Img } from "../core/image.ts";
import type { Project } from "../core/project.ts";
import { writePng } from "./output.ts";

export interface FeedbackNote { x: number; y: number; text: string }
export interface FeedbackBody {
  name: string;
  w: number;
  h: number;
  /** Overlay pixels, packed like asset images (base64 of a Uint32Array); 0 = no mark. */
  marks: string;
  notes?: FeedbackNote[];
  comment?: string;
  /** The viewer's rendering of image + marks + numbered pins (data:image/png;base64,...). */
  annotated?: string;
}

const MAX_NOTES = 200, MAX_TEXT = 4000;

export class FeedbackError extends Error {}

/** Validate and write `<out>/_feedback/<name>.{json,marks.png,current.png,annotated.png}`. */
export function writeFeedback(p: Project, out: string, body: unknown): { files: string[]; summary: string } {
  if (typeof body !== "object" || body === null) throw new FeedbackError("feedback must be a JSON object");
  const b = body as Partial<FeedbackBody>;
  const def = typeof b.name === "string" ? p.assets.get(b.name) : undefined;
  if (!def || !b.name) throw new FeedbackError(`unknown asset ${JSON.stringify(b.name)}`);
  const name = b.name;
  const img = p.image(name);
  if (b.w !== img.w || b.h !== img.h) throw new FeedbackError(`marks are ${b.w}x${b.h} but ${name} is ${img.w}x${img.h}`);
  if (typeof b.marks !== "string") throw new FeedbackError("marks must be base64 pixels");
  const raw = Buffer.from(b.marks, "base64");
  if (raw.length !== img.w * img.h * 4) throw new FeedbackError(`marks hold ${raw.length / 4} pixels, expected ${img.w * img.h}`);
  const marks = new Uint32Array(raw.buffer.slice(raw.byteOffset, raw.byteOffset + raw.length));

  const notes = (Array.isArray(b.notes) ? b.notes : []).slice(0, MAX_NOTES).flatMap((n) =>
    n && Number.isInteger(n.x) && Number.isInteger(n.y) && n.x >= 0 && n.y >= 0 && n.x < img.w && n.y < img.h && typeof n.text === "string"
      ? [{ x: n.x, y: n.y, text: n.text.slice(0, MAX_TEXT) }] : []);
  const comment = typeof b.comment === "string" ? b.comment.slice(0, MAX_TEXT) : "";

  // Painted pixels grouped by color; palette colors are "make it this", the rest are pointers.
  const rev = def.pal.reverse();
  const groups = new Map<Color, [number, number][]>();
  for (let i = 0; i < marks.length; i++) {
    const c = marks[i];
    if (c === TRANSPARENT) continue;
    let g = groups.get(c);
    if (!g) groups.set(c, (g = []));
    g.push([i % img.w, Math.floor(i / img.w)]);
  }
  const under = (x: number, y: number) => {
    const c = img.get(x, y);
    return c === TRANSPARENT ? { hex: "transparent", key: "." } : { hex: toHex(c), key: rev.get(c) ?? null };
  };
  const paint = [...groups].map(([c, px]) => {
    const xs = px.map((q) => q[0]), ys = px.map((q) => q[1]);
    const key = rev.get(c) ?? null;
    return { hex: toHex(c), key, kind: key === null ? "markup" : "palette", count: px.length,
      bbox: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)], pixels: px };
  }).sort((a, z) => z.count - a.count);

  const dir = join(out, "_feedback");
  mkdirSync(dir, { recursive: true });
  const files: string[] = [];
  const put = (file: string, write: (path: string) => void) => { const path = join(dir, file); write(path); files.push(path); };
  put(`${name}.current.png`, (f) => writePng(f, img));
  put(`${name}.marks.png`, (f) => writePng(f, new Img(img.w, img.h, marks)));
  if (typeof b.annotated === "string" && b.annotated.startsWith("data:image/png;base64,"))
    put(`${name}.annotated.png`, (f) => writeFileSync(f, Buffer.from(b.annotated!.slice(22), "base64")));
  put(`${name}.json`, (f) => writeFileSync(f, JSON.stringify({
    asset: name, kind: def.kind, size: [img.w, img.h], time: new Date().toISOString(), comment,
    notes: notes.map((n, i) => ({ n: i + 1, ...n, under: under(n.x, n.y) })),
    paint,
    images: { current: `${name}.current.png`, marks: `${name}.marks.png`, annotated: files.some((x) => x.endsWith(".annotated.png")) ? `${name}.annotated.png` : null },
  }, null, 1) + "\n"));
  const painted = paint.reduce((a, g) => a + g.count, 0);
  return { files, summary: `${notes.length} note${notes.length === 1 ? "" : "s"}, ${painted} painted pixel${painted === 1 ? "" : "s"}${comment ? ", a comment" : ""}` };
}
