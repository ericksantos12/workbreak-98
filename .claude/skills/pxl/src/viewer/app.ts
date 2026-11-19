// pxl live viewer. Served type-stripped as /app.js; no imports, no build step.

interface ErrInfo { message: string; loc?: string }
interface AssetInfo { name: string; kind: string; part: boolean; w?: number; h?: number; hash?: string; steps?: number; changed: boolean; edited: boolean; error?: ErrInfo }
interface State { version: number; file: string; time: string; ms: number; error?: ErrInfo; assets: AssetInfo[]; sheets: string[]; errors: ErrInfo[] }
interface FrameJ { label: string; loc?: string; w: number; h: number; full?: string; changes?: string }
interface AssetJ { name: string; kind: string; w: number; h: number; loc?: string; palette: Record<string, string>; layers: string[]; image: string; frames: FrameJ[]; anim?: { ms: number; image: string }[] }
interface Frame { label: string; loc?: string; w: number; h: number; full?: Uint32Array; changes?: Uint32Array }
interface Pic { w: number; h: number; data: Uint32Array }
interface Loaded { j: AssetJ; hash?: string; frames: Frame[]; final: Pic; marks: Map<number, Pic>; rev: Map<number, string>; anim: { ms: number; pic: Pic }[] }

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const view = $<HTMLCanvasElement>("view");
const g = view.getContext("2d")!;
const slider = $<HTMLInputElement>("slider");

let state: State | undefined;
let sel: string | undefined;
let cur: Loaded | undefined;
let step = 0;
let playing = false;
let zoom = 0; // 0 = fit
let pan = { x: 0, y: 0 };
let hover: { x: number; y: number } | undefined;
// last drawn layout, so mouse positions map to asset pixels
let layout = { ox: 0, oy: 0, z: 1, dpr: 1, w: 0, h: 0 };
const thumbs = new Map<string, string>(); // name -> hash rendered

// ---------------------------------------------------------------- decoding

function u32(b64: string): Uint32Array {
  const bin = atob(b64);
  const u8 = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
  return new Uint32Array(u8.buffer);
}

function toImageData(p: Pic): ImageData {
  const id = new ImageData(p.w, p.h);
  const d = id.data;
  for (let i = 0; i < p.data.length; i++) {
    const c = p.data[i];
    d[i * 4] = c >>> 24; d[i * 4 + 1] = (c >>> 16) & 255; d[i * 4 + 2] = (c >>> 8) & 255; d[i * 4 + 3] = c & 255;
  }
  return id;
}

function hex(c: number): string {
  return "#" + (c >>> 8).toString(16).padStart(6, "0") + ((c & 255) === 255 ? "" : (c & 255).toString(16).padStart(2, "0"));
}

/** Image after step n, rebuilt from the nearest checkpoint. */
function picAt(a: Loaded, n: number): Pic {
  if (!a.frames.length) return a.final;
  n = Math.max(0, Math.min(n, a.frames.length - 1));
  let k = n;
  while (k > 0 && !a.marks.has(k)) k--;
  let pic = a.marks.get(k);
  if (!pic) {
    const f = a.frames[0];
    pic = { w: f.w, h: f.h, data: f.full ? f.full.slice() : new Uint32Array(f.w * f.h) };
    a.marks.set(0, { ...pic, data: pic.data.slice() });
  } else pic = { ...pic, data: pic.data.slice() };
  for (let i = k + 1; i <= n; i++) {
    const f = a.frames[i];
    if (f.full) pic = { w: f.w, h: f.h, data: f.full.slice() };
    else if (f.changes) for (let j = 0; j < f.changes.length; j += 2) pic.data[f.changes[j]] = f.changes[j + 1];
    if (i % 32 === 0 && !a.marks.has(i)) a.marks.set(i, { ...pic, data: pic.data.slice() });
  }
  return pic;
}

// ---------------------------------------------------------------- loading

async function fetchAsset(name: string, frames: boolean): Promise<AssetJ | undefined> {
  const r = await fetch(`/api/asset?name=${encodeURIComponent(name)}&frames=${frames ? 1 : 0}`);
  return r.ok ? (r.json() as Promise<AssetJ>) : undefined;
}

async function select(name: string, opts: { play?: boolean; keepStep?: boolean } = {}): Promise<void> {
  const info = state?.assets.find((a) => a.name === name);
  sel = name;
  localStorage.setItem("pxl.sel", name);
  renderList();
  if (!info || info.error) {
    cur = undefined;
    renderSteps();
    draw();
    return;
  }
  const j = await fetchAsset(name, true);
  if (!j || sel !== name) return;
  const frames: Frame[] = j.frames.map((f) => ({ label: f.label, loc: f.loc, w: f.w, h: f.h, full: f.full ? u32(f.full) : undefined, changes: f.changes ? u32(f.changes) : undefined }));
  const rev = new Map<number, string>();
  for (const [k, v] of Object.entries(j.palette)) {
    const c = parseInt(v.slice(1).padEnd(8, "f"), 16) >>> 0;
    if (!rev.has(c) || /[A-Za-z]/.test(k)) rev.set(c, k);
  }
  const prevStep = step, prevLen = cur?.frames.length ?? 0;
  const anim = (j.anim ?? []).map((f) => ({ ms: f.ms, pic: { w: j.w, h: j.h, data: u32(f.image) } }));
  cur = { j, hash: info.hash, frames, final: { w: j.w, h: j.h, data: u32(j.image) }, marks: new Map(), rev, anim };
  animAt = 0;
  slider.max = String(Math.max(0, frames.length - 1));
  step = opts.play ? 0 : opts.keepStep && prevStep < prevLen - 1 ? Math.min(prevStep, frames.length - 1) : frames.length - 1;
  renderSteps();
  renderPalette();
  if (annotating) renderAnnot();
  setStep(step);
  if (opts.play) play(true);
}

// ---------------------------------------------------------------- list

function renderList(): void {
  const box = $("assets");
  const q = ($<HTMLInputElement>("filter")).value.trim().toLowerCase();
  box.innerHTML = "";
  if (!state) return;
  const groups: [string, AssetInfo[]][] = [
    ["scenes", state.assets.filter((a) => a.kind === "scene")],
    ["anims", state.assets.filter((a) => a.kind === "anim" && !a.part)],
    ["sprites", state.assets.filter((a) => a.kind === "sprite" && !a.part)],
    ["parts", state.assets.filter((a) => a.part)],
  ];
  for (const [title, list] of groups) {
    const shown = list.filter((a) => !q || a.name.toLowerCase().includes(q));
    if (!shown.length) continue;
    const h = document.createElement("div");
    h.className = "sect";
    h.textContent = `${title} · ${shown.length}`;
    box.append(h);
    for (const a of shown) {
      const row = document.createElement("div");
      row.className = "row" + (a.name === sel ? " sel" : "");
      const cv = document.createElement("canvas");
      cv.width = 32; cv.height = 32;
      cv.dataset.name = a.name;
      const meta = document.createElement("div");
      meta.innerHTML = `<div class="nm"></div><div class="sz">${a.error ? "error" : `${a.w}×${a.h} · ${a.steps} steps`}</div>`;
      (meta.firstChild as HTMLElement).textContent = a.name;
      row.append(cv, meta);
      if (a.error || (a.changed && state.version > 1)) {
        const dot = document.createElement("span");
        dot.className = "dot " + (a.error ? "err" : "new");
        row.append(dot);
      }
      row.onclick = () => select(a.name);
      box.append(row);
      paintThumb(cv, a);
    }
  }
}

const thumbCache = new Map<string, Pic>();
async function paintThumb(cv: HTMLCanvasElement, a: AssetInfo): Promise<void> {
  if (a.error || !a.hash) return;
  let pic = thumbs.get(a.name) === a.hash ? thumbCache.get(a.name) : undefined;
  if (!pic) {
    const j = await fetchAsset(a.name, false);
    if (!j) return;
    pic = { w: j.w, h: j.h, data: u32(j.image) };
    thumbs.set(a.name, a.hash);
    thumbCache.set(a.name, pic);
  }
  const off = new OffscreenCanvas(pic.w, pic.h);
  off.getContext("2d")!.putImageData(toImageData(pic), 0, 0);
  const c = cv.getContext("2d")!;
  c.imageSmoothingEnabled = false;
  const s = Math.min(32 / pic.w, 32 / pic.h);
  const w = pic.w * s, h = pic.h * s;
  c.clearRect(0, 0, 32, 32);
  c.drawImage(off, (32 - w) / 2, (32 - h) / 2, w, h);
}

// ---------------------------------------------------------------- steps panel

function renderSteps(): void {
  const ol = $("steps");
  ol.innerHTML = "";
  if (!cur) return;
  const frag = document.createDocumentFragment();
  cur.frames.forEach((f, i) => {
    const li = document.createElement("li");
    const none = f.changes && f.changes.length === 0;
    li.className = none ? "none" : "";
    li.title = (f.loc ? f.loc + "\n" : "") + f.label + (none ? "\n(no visible change)" : "");
    li.innerHTML = `<span class="i">${i}</span><span class="l"></span>`;
    (li.lastChild as HTMLElement).textContent = f.label;
    li.onclick = () => { play(false); setStep(i); };
    frag.append(li);
  });
  ol.append(frag);
}

function renderPalette(): void {
  const box = $("pal");
  box.innerHTML = "";
  if (!cur) return;
  for (const [k, v] of Object.entries(cur.j.palette)) {
    const s = document.createElement("span");
    s.style.background = v;
    s.textContent = k;
    s.title = `${k} ${v} — click to paint with it`;
    s.onclick = () => { if (!annotating) setAnnotate(true); brush = parseInt(v.slice(1).padEnd(8, "f"), 16) >>> 0; tool = "brush"; renderAnnot(); };
    box.append(s);
  }
}

function setStep(n: number): void {
  if (!cur) return draw();
  step = Math.max(0, Math.min(n, Math.max(0, cur.frames.length - 1)));
  slider.value = String(step);
  const ol = $("steps");
  ol.querySelector(".cur")?.classList.remove("cur");
  const li = ol.children[step] as HTMLElement | undefined;
  if (li) { li.classList.add("cur"); li.scrollIntoView({ block: "nearest" }); }
  const f = cur.frames[step];
  const info = $("stepinfo");
  info.innerHTML = f
    ? `step <b>${step}</b>/${cur.frames.length - 1} · <b></b> <span style="opacity:.7"></span>`
    : `${cur.j.name} (no steps)`;
  if (f) {
    (info.children[1] as HTMLElement).textContent = f.label;
    (info.children[2] as HTMLElement).textContent = f.loc ? `@ ${f.loc}` : "";
  }
  draw();
}

// ---------------------------------------------------------------- drawing

function draw(): void {
  const dpr = window.devicePixelRatio || 1;
  const W = Math.floor(view.clientWidth * dpr), H = Math.floor(view.clientHeight * dpr);
  if (view.width !== W || view.height !== H) { view.width = W; view.height = H; }
  g.setTransform(1, 0, 0, 1, 0, 0);
  g.clearRect(0, 0, W, H);
  showError();
  if (!cur) { $("hud").textContent = ""; return; }

  // annotating always shows the finished image (marks are in its pixels; the replay can be capped)
  const pic = annotating ? cur.final : animating() ? cur.anim[animAt].pic : picAt(cur, step);
  const fit = Math.max(1, Math.min(48, Math.floor(Math.min((W - 40) / pic.w, (H - 40) / pic.h))));
  const z = zoom || fit;
  const ox = Math.floor((W - pic.w * z) / 2 + pan.x * dpr), oy = Math.floor((H - pic.h * z) / 2 + pan.y * dpr);

  // checkerboard, one check per pixel
  for (let y = 0; y < pic.h; y++)
    for (let x = 0; x < pic.w; x++) {
      g.fillStyle = (x + y) % 2 ? "#2a2735" : "#33303f";
      g.fillRect(ox + x * z, oy + y * z, z, z);
    }
  const off = new OffscreenCanvas(pic.w, pic.h);
  off.getContext("2d")!.putImageData(toImageData(pic), 0, 0);
  g.imageSmoothingEnabled = false;
  g.drawImage(off, ox, oy, pic.w * z, pic.h * z);

  if (($<HTMLInputElement>("gridtog")).checked && z >= 6) {
    g.beginPath();
    for (let x = 0; x <= pic.w; x++) if (x % 8) { g.moveTo(ox + x * z + 0.5, oy); g.lineTo(ox + x * z + 0.5, oy + pic.h * z); }
    for (let y = 0; y <= pic.h; y++) if (y % 8) { g.moveTo(ox, oy + y * z + 0.5); g.lineTo(ox + pic.w * z, oy + y * z + 0.5); }
    g.strokeStyle = "rgba(0,0,0,.22)"; g.lineWidth = 1; g.stroke();
    g.beginPath();
    for (let x = 0; x <= pic.w; x += 8) { g.moveTo(ox + x * z + 0.5, oy); g.lineTo(ox + x * z + 0.5, oy + pic.h * z); }
    for (let y = 0; y <= pic.h; y += 8) { g.moveTo(ox, oy + y * z + 0.5); g.lineTo(ox + pic.w * z, oy + y * z + 0.5); }
    g.strokeStyle = "rgba(0,0,0,.5)"; g.stroke();
  }

  // highlight what the current step changed: tint + bounding box
  const f = cur.frames[step];
  if (($<HTMLInputElement>("hl")).checked && f?.changes && f.changes.length && step < cur.frames.length - 1 + (playing ? 1 : 0)) {
    let x0 = Infinity, y0 = Infinity, x1 = -1, y1 = -1;
    g.fillStyle = "rgba(255,180,84,.42)";
    for (let j = 0; j < f.changes.length; j += 2) {
      const i = f.changes[j], x = i % f.w, y = Math.floor(i / f.w);
      g.fillRect(ox + x * z, oy + y * z, z, z);
      x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
    }
    g.strokeStyle = "#ffb454"; g.lineWidth = Math.max(1, dpr);
    g.strokeRect(ox + x0 * z - 1.5, oy + y0 * z - 1.5, (x1 - x0 + 1) * z + 3, (y1 - y0 + 1) * z + 3);
  }

  layout = { ox, oy, z, dpr, w: pic.w, h: pic.h };
  drawMarks(ox, oy, z, dpr, pic);
  $("hud").textContent = `${cur.j.kind} ${cur.j.name} · ${pic.w}×${pic.h} · ${z}×${zoom ? "" : " (fit)"}` +
    (cur.anim.length ? ` · frame ${animating() ? animAt : 0}/${cur.anim.length - 1}` : "");
  inspect(pic, ox, oy, z, dpr);
}

function inspect(pic: Pic, ox: number, oy: number, z: number, dpr: number): void {
  const box = $("inspect");
  if (!hover) { box.textContent = "hover the image"; return; }
  const x = Math.floor((hover.x * dpr - ox) / z), y = Math.floor((hover.y * dpr - oy) / z);
  if (x < 0 || y < 0 || x >= pic.w || y >= pic.h) { box.textContent = "hover the image"; return; }
  const c = pic.data[y * pic.w + x];
  const key = c === 0 ? "." : cur?.rev.get(c) ?? "?";
  box.innerHTML = `<span class="sw"></span><span>x <b>${x}</b> y <b>${y}</b><br>${c === 0 ? "transparent" : hex(c)} · key <b></b></span>`;
  (box.firstChild as HTMLElement).style.background = c === 0 ? "transparent" : hex(c);
  (box.querySelector("b:last-child") as HTMLElement).textContent = key;
}

function showError(): void {
  const el = $("err");
  const info = state?.assets.find((a) => a.name === sel);
  const err = state?.error ?? info?.error ?? (state?.errors.length ? state.errors[0] : undefined);
  el.style.display = err ? "block" : "none";
  if (err) el.textContent = err.message;
}

// ---------------------------------------------------------------- playback

let last = 0, acc = 0;
function play(on: boolean): void {
  playing = on;
  $("play").textContent = on ? "⏸" : "▶";
  if (on && cur && step >= cur.frames.length - 1) setStep(0);
  last = performance.now();
  acc = 0;
  if (on) requestAnimationFrame(tick);
}

function tick(t: number): void {
  if (!playing || !cur) return;
  const sp = ($<HTMLSelectElement>("speed")).value;
  const n = cur.frames.length;
  const rate = sp === "auto" ? Math.max(8, n / 3) : Number(sp); // auto: whole build in ~3s
  acc += ((t - last) / 1000) * rate;
  last = t;
  if (acc >= 1) {
    const adv = Math.floor(acc);
    acc -= adv;
    if (step + adv >= n - 1) { setStep(n - 1); play(false); return; }
    setStep(step + adv);
  }
  requestAnimationFrame(tick);
}

// ---------------------------------------------------------------- animation
// Animated assets loop through their frames once the painting replay has reached the end.

let animAt = 0, animTimer = 0;
function animating(): boolean {
  return !!cur && cur.anim.length > 1 && $<HTMLInputElement>("animtog").checked && !playing && step >= cur.frames.length - 1;
}

function animTick(): void {
  clearTimeout(animTimer);
  if (animating()) {
    animAt = (animAt + 1) % cur!.anim.length;
    draw();
  }
  const ms = cur?.anim[animAt]?.ms ?? 100;
  animTimer = window.setTimeout(animTick, Math.max(20, ms));
}
animTick();

// ---------------------------------------------------------------- events

function connect(): void {
  const es = new EventSource("/events");
  es.addEventListener("build", (ev) => onBuild(JSON.parse((ev as MessageEvent).data) as State));
  es.onerror = () => { const s = $("status"); s.textContent = "disconnected"; s.className = "pill bad"; };
}

function onBuild(s: State): void {
  const first = !state;
  state = s;
  $("file").textContent = s.file;
  const bad = s.error ? 1 : s.assets.filter((a) => a.error).length + s.errors.length;
  const st = $("status");
  st.textContent = bad ? `${bad} error${bad > 1 ? "s" : ""}` : "ok";
  st.className = "pill " + (bad ? "bad" : "ok");
  $("buildinfo").textContent = `v${s.version} · ${s.ms}ms · ${s.time}`;
  renderList();

  const follow = ($<HTMLInputElement>("follow")).checked;
  const changed = s.assets.filter((a) => a.changed);
  if (first) {
    const want = localStorage.getItem("pxl.sel");
    const pick = s.assets.find((a) => a.name === want) ?? s.assets.find((a) => a.kind === "scene") ?? s.assets.find((a) => !a.part) ?? s.assets[0];
    if (pick) select(pick.name);
    else draw();
    return;
  }
  if (follow && changed.length) {
    // follow what was edited (errors first), else the selected asset, else the first changed one
    const edited = s.assets.filter((a) => a.edited);
    const target = edited.find((a) => a.error) ?? edited.find((a) => a.name === sel) ?? edited.find((a) => !a.part) ?? edited[0]
      ?? changed.find((a) => a.name === sel) ?? changed.find((a) => !a.part) ?? changed[0];
    select(target.name, { play: !target.error });
  } else if (sel) {
    const info = s.assets.find((a) => a.name === sel);
    if (!info) draw();
    else if (info.hash !== cur?.hash || info.error) select(sel, { keepStep: true });
    else draw();
  }
}

slider.oninput = () => { play(false); setStep(Number(slider.value)); };
$("play").onclick = () => play(!playing);
$("first").onclick = () => { play(false); setStep(0); };
$("last").onclick = () => { play(false); setStep(Infinity); };
$("prev").onclick = () => { play(false); setStep(step - 1); };
$("next").onclick = () => { play(false); setStep(step + 1); };
$("filter").oninput = renderList;
for (const id of ["gridtog", "hl", "animtog"]) $(id).onchange = draw;

const stage = $("stage");
stage.addEventListener("wheel", (e) => {
  e.preventDefault();
  if (!cur) return;
  const dpr = window.devicePixelRatio || 1;
  const pic = picAt(cur, step);
  const fit = Math.max(1, Math.min(48, Math.floor(Math.min((view.width - 40) / pic.w, (view.height - 40) / pic.h))));
  const z = zoom || fit;
  zoom = Math.max(1, Math.min(64, e.deltaY < 0 ? Math.ceil(z * 1.25) : Math.floor(z / 1.25)));
  void dpr;
  draw();
}, { passive: false });
let drag: { x: number; y: number; px: number; py: number } | undefined;
stage.addEventListener("contextmenu", (e) => { if (annotating) e.preventDefault(); });
stage.addEventListener("mousedown", (e) => {
  if (annotating && !e.shiftKey && (e.button === 0 || e.button === 2)) { annotDown(e); return; }
  drag = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
});
window.addEventListener("mouseup", () => { drag = undefined; stroke = undefined; });
stage.addEventListener("mousemove", (e) => {
  const r = stage.getBoundingClientRect();
  hover = { x: e.clientX - r.left, y: e.clientY - r.top };
  if (stroke) annotMove(e);
  else if (drag) pan = { x: drag.px + e.clientX - drag.x, y: drag.py + e.clientY - drag.y };
  draw();
});
stage.addEventListener("mouseleave", () => { hover = undefined; draw(); });
window.addEventListener("resize", draw);
window.addEventListener("keydown", (e) => {
  const tag = (e.target as HTMLElement).tagName;
  if ((tag === "INPUT" && (e.target as HTMLInputElement).type === "text") || tag === "TEXTAREA") return;
  if (e.key === "a") { setAnnotate(!annotating); return; }
  if (annotating && "bne".includes(e.key)) { tool = e.key === "b" ? "brush" : e.key === "n" ? "note" : "erase"; renderAnnot(); return; }
  if (e.key === " ") { e.preventDefault(); play(!playing); }
  else if (e.key === "ArrowLeft") { play(false); setStep(step - 1); }
  else if (e.key === "ArrowRight") { play(false); setStep(step + 1); }
  else if (e.key === "Home") { play(false); setStep(0); }
  else if (e.key === "End") { play(false); setStep(Infinity); }
  else if (e.key === "f") { zoom = 0; pan = { x: 0, y: 0 }; draw(); }
  else if (e.key === "g") { const t = $<HTMLInputElement>("gridtog"); t.checked = !t.checked; draw(); }
  else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
    e.preventDefault();
    const names = [...document.querySelectorAll<HTMLCanvasElement>("#assets canvas")].map((c) => c.dataset.name!);
    const i = names.indexOf(sel ?? "");
    const n = names[Math.max(0, Math.min(names.length - 1, i + (e.key === "ArrowUp" ? -1 : 1)))];
    if (n) select(n);
  }
});

// ---------------------------------------------------------------- annotate: feedback for Claude
// Marks are an overlay in asset pixels (same packing as images), per asset, kept in localStorage
// until sent. Markup colors point at things; palette colors mean "make it this color".

interface Marks { w: number; h: number; data: Uint32Array; notes: { x: number; y: number; text: string }[]; comment: string }
const MARKUP = [0xff3df0ff, 0x2ee6ffff, 0xffe14aff, 0xffffffff];
let annotating = false, tool: "brush" | "note" | "erase" = "brush", brush = MARKUP[0], size = 1;
let stroke: { erase: boolean; last?: [number, number] } | undefined;
const marks = new Map<string, Marks>();

function marksFor(create: boolean): Marks | undefined {
  if (!cur) return undefined;
  const key = cur.j.name;
  let m = marks.get(key);
  if (m && (m.w !== cur.final.w || m.h !== cur.final.h)) { marks.delete(key); m = undefined; }   // asset resized
  if (!m) {
    try {
      const saved = JSON.parse(localStorage.getItem("pxl.marks." + key) ?? "null");
      if (saved && saved.w === cur.final.w && saved.h === cur.final.h) m = { ...saved, data: u32(saved.data) };
    } catch { /* ignore */ }
  }
  if (!m && create) m = { w: cur.final.w, h: cur.final.h, data: new Uint32Array(cur.final.w * cur.final.h), notes: [], comment: "" };
  if (m) marks.set(key, m);
  return m;
}
function b64u32(a: Uint32Array): string {
  let s = ""; const u8 = new Uint8Array(a.buffer, a.byteOffset, a.byteLength);
  for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000));
  return btoa(s);
}
function saveMarks(): void {
  const m = marksFor(false);
  if (!m || !cur) return;
  try { localStorage.setItem("pxl.marks." + cur.j.name, JSON.stringify({ ...m, data: b64u32(m.data) })); } catch { /* full or blocked */ }
}

function setAnnotate(on: boolean): void {
  annotating = on;
  document.body.classList.toggle("annotating", on);
  $("annotbtn").classList.toggle("on", on);
  if (on) { $<HTMLInputElement>("animtog").checked = false; play(false); setStep(Infinity); }
  renderAnnot();
  draw();
}

function renderAnnot(): void {
  for (const t of ["brush", "note", "erase"]) $("t-" + t).classList.toggle("on", tool === t);
  $("t-size").textContent = `${size}px`;
  const box = $("colors");
  box.innerHTML = "";
  for (const c of MARKUP.concat(MARKUP.includes(brush) ? [] : [brush])) {
    const b = document.createElement("button");
    b.className = "chip" + (c === brush && tool === "brush" ? " on" : "");
    b.style.background = hex(c);
    b.title = MARKUP.includes(c) ? "markup color" : "palette color: make it this";
    b.onclick = () => { brush = c; tool = "brush"; renderAnnot(); };
    box.append(b);
  }
  const m = marksFor(false);
  const ol = $("notes");
  ol.innerHTML = "";
  m?.notes.forEach((n, i) => {
    const li = document.createElement("li");
    li.innerHTML = `<b>${i + 1}</b><span></span><a title="remove">×</a>`;
    (li.children[1] as HTMLElement).textContent = `(${n.x},${n.y}) ${n.text}`;
    (li.children[2] as HTMLElement).onclick = () => { m.notes.splice(i, 1); saveMarks(); renderAnnot(); draw(); };
    ol.append(li);
  });
  $<HTMLTextAreaElement>("comment").value = m?.comment ?? "";
}

function pixelAt(e: MouseEvent): [number, number] | undefined {
  const r = stage.getBoundingClientRect();
  const x = Math.floor(((e.clientX - r.left) * layout.dpr - layout.ox) / layout.z), y = Math.floor(((e.clientY - r.top) * layout.dpr - layout.oy) / layout.z);
  return x >= 0 && y >= 0 && x < layout.w && y < layout.h ? [x, y] : undefined;
}

function dab(m: Marks, x: number, y: number, erase: boolean): void {
  const o = Math.floor((size - 1) / 2);
  for (let dy = 0; dy < size; dy++) for (let dx = 0; dx < size; dx++) {
    const px = x - o + dx, py = y - o + dy;
    if (px >= 0 && py >= 0 && px < m.w && py < m.h) m.data[py * m.w + px] = erase ? 0 : brush;
  }
}

function annotDown(e: MouseEvent): void {
  const p = pixelAt(e), m = marksFor(true);
  if (!p || !m) return;
  if (tool === "note" && e.button === 0) {
    const text = prompt(`Note for Claude at pixel ${p[0]}, ${p[1]}:`);
    if (text && text.trim()) { m.notes.push({ x: p[0], y: p[1], text: text.trim() }); saveMarks(); renderAnnot(); draw(); }
    return;
  }
  stroke = { erase: tool === "erase" || e.button === 2, last: p };
  dab(m, p[0], p[1], stroke.erase);
  draw();
}

function annotMove(e: MouseEvent): void {
  const p = pixelAt(e), m = marksFor(true);
  if (!p || !m || !stroke) return;
  let [x0, y0] = stroke.last ?? p;
  const [x1, y1] = p, dx = Math.abs(x1 - x0), dy = -Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  for (;;) {                                         // Bresenham, so fast strokes stay connected
    dab(m, x0, y0, stroke.erase);
    if (x0 === x1 && y0 === y1) break;
    const e2 = 2 * err;
    if (e2 >= dy) { err += dy; x0 += sx; }
    if (e2 <= dx) { err += dx; y0 += sy; }
  }
  stroke.last = p;
  saveMarks();
}

function drawMarks(ox: number, oy: number, z: number, dpr: number, pic: Pic): void {
  const m = marksFor(false);
  if (!m || m.w !== pic.w || m.h !== pic.h) return;
  const off = new OffscreenCanvas(m.w, m.h);
  off.getContext("2d")!.putImageData(toImageData({ w: m.w, h: m.h, data: m.data }), 0, 0);
  g.globalAlpha = annotating ? 0.9 : 0.55;
  g.drawImage(off, ox, oy, m.w * z, m.h * z);
  g.globalAlpha = 1;
  pins(g, m, ox, oy, z, dpr);
}

function pins(c: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, m: Marks, ox: number, oy: number, z: number, dpr: number): void {
  const r = Math.max(7 * dpr, z * 0.9);
  c.font = `700 ${Math.round(r * 1.1)}px ui-monospace, monospace`;
  c.textAlign = "center"; c.textBaseline = "middle";
  m.notes.forEach((n, i) => {
    const cx = ox + (n.x + 0.5) * z, cy = oy + (n.y + 0.5) * z;
    c.strokeStyle = "#ffb454"; c.lineWidth = Math.max(1.5, dpr * 1.5);
    c.strokeRect(ox + n.x * z - 1, oy + n.y * z - 1, z + 2, z + 2);
    c.beginPath(); c.arc(cx + r * 1.2, cy - r * 1.2, r, 0, Math.PI * 2);
    c.fillStyle = "#ffb454"; c.fill();
    c.fillStyle = "#15131c"; c.fillText(String(i + 1), cx + r * 1.2, cy - r * 1.2 + 1);
  });
}

async function sendFeedback(): Promise<void> {
  const m = marksFor(false), st = $("fbstatus");
  if (!cur || !m) { st.textContent = "nothing to send yet"; return; }
  // The annotated image: the asset scaled up, marks on top, numbered pins.
  const s = Math.max(1, Math.floor(900 / Math.max(m.w, m.h)));
  const cv = new OffscreenCanvas(m.w * s, m.h * s), c = cv.getContext("2d")!;
  c.imageSmoothingEnabled = false;
  const img = new OffscreenCanvas(m.w, m.h); img.getContext("2d")!.putImageData(toImageData(cur.final), 0, 0);
  const mk = new OffscreenCanvas(m.w, m.h); mk.getContext("2d")!.putImageData(toImageData({ w: m.w, h: m.h, data: m.data }), 0, 0);
  c.fillStyle = "#0e0d13"; c.fillRect(0, 0, cv.width, cv.height);
  c.drawImage(img, 0, 0, cv.width, cv.height);
  c.globalAlpha = 0.9; c.drawImage(mk, 0, 0, cv.width, cv.height); c.globalAlpha = 1;
  pins(c, m, 0, 0, s, 1.4);
  const blob = await cv.convertToBlob({ type: "image/png" });
  const annotated = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(blob); });
  st.textContent = "sending…";
  const r = await fetch("/api/feedback", { method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: cur.j.name, w: m.w, h: m.h, marks: b64u32(m.data), notes: m.notes, comment: m.comment, annotated }) });
  const j = await r.json().catch(() => ({ error: `HTTP ${r.status}` }));
  st.textContent = r.ok ? `sent ✓ ${j.summary} — tell Claude to check the feedback` : `not sent: ${j.error}`;
}

$("annotbtn").onclick = () => setAnnotate(!annotating);
$("t-brush").onclick = () => { tool = "brush"; renderAnnot(); };
$("t-note").onclick = () => { tool = "note"; renderAnnot(); };
$("t-erase").onclick = () => { tool = "erase"; renderAnnot(); };
$("t-size").onclick = () => { size = size === 1 ? 2 : size === 2 ? 4 : 1; renderAnnot(); };
$("t-clear").onclick = () => {
  if (!cur || !confirm(`Clear all marks and notes on ${cur.j.name}?`)) return;
  marks.delete(cur.j.name); localStorage.removeItem("pxl.marks." + cur.j.name); renderAnnot(); draw();
};
$("comment").oninput = () => { const m = marksFor(true); if (m) { m.comment = ($<HTMLTextAreaElement>("comment")).value; saveMarks(); } };
$("send").onclick = () => { void sendFeedback(); };

connect();
