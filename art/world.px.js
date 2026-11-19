// WorkBreak 98 world: side-scrolling parallax layers and the campsite.
// Original art, loosely inspired by bright fantasy pixel landscapes.
//
// Build: node .claude/skills/pxl/bin/pxl.js build art/world.px.js --out art/out
// Then copy art/out/scenes/world_{sky,clouds,mountains,hills,ground,camp}.png and
// art/out/anims/camp_fire.png to src/assets/hero/ (preview_* scenes are not shipped).
//
// Every layer is 160x48 and wraps horizontally (whatever leaves the right edge comes back
// on the left), so the UI can scroll it with repeat-x at different speeds. The ground top
// is row 40, where the hero stands.

pal({
  k: "#1b1630",
  S: "#3f8fd8", s: "#6cb6ec", z: "#a9dcf5",              // sky, top -> horizon
  o: "#ffffff", O: "#d5e6f3",                            // clouds
  m: "#5d86b8", M: "#7ea4cf", n: "#b9d3ea", N: "#eef5fb", // far mountains and snow
  G: "#2e6b3e", g: "#4c9a4a", h: "#7cc35a", H: "#245232", // greens
  e: "#7b5131", E: "#5a3a26", d: "#9a6a3d", D: "#b98852", // dirt
  r: "#8d8a96", R: "#5f5d6b",                            // stones
  t: "#6b4528", T: "#4a2f1d",                            // trunks and logs
  F: "#c8321e", f: "#f07a24", y: "#ffd34e", v: "#fff3b0", // fire
  i: "#9e2b2b",                                          // tent interior
})
ramps("HGgh", "EedD", "Rr", "mMnN", "Tt", "Ffyv")

const WW = 160, WH = 48, GROUND = 40
const TAU = Math.PI * 2
// A wave that repeats exactly every WW pixels.
const wave = (x, parts) => parts.reduce((sum, [n, a, p]) => sum + a * Math.sin((x / WW) * TAU * n + p), 0)
// Draw an op at x and again one period left/right so shapes crossing an edge wrap around.
const wrapX = (x, draw) => [x - WW, x, x + WW].forEach(draw)

// Sky gradient (static, does not scroll).
scene("world_sky", { w: WW, h: WH }, grad(0, 0, WW, 34, "Ssz", "v", 0.6), rect(0, 34, WW, 14, "z"))

// Clouds: puffy, slow.
const cloud = (c, x, y, w) => wrapX(x, (cx) => {
  c.ellipse(cx, y, w, 3, "o")
  c.ellipse(cx - Math.round(w / 2), y + 1, Math.round(w / 2), 2, "o")
  c.ellipse(cx + Math.round(w / 3), y - 2, Math.round(w / 2), 3, "o")
  c.hline(cx - w + 1, y + 3, w * 2 - 1, "O")
})
scene("world_clouds", { w: WW, h: WH }, (c) => {
  cloud(c, 24, 8, 9)
  cloud(c, 92, 5, 7)
  cloud(c, 134, 12, 6)
})

// Far mountains with snowy peaks, lit from the left.
scene("world_mountains", { w: WW, h: WH }, (c) => {
  const top = (x) => Math.round(23 + wave(x, [[2, 8, 0.4], [3, 4, 1.7], [7, 1.5, 0.2]]))
  for (let x = 0; x < WW; x++) {
    const y = top(x)
    const rising = top(x + 1) < y || top(x - 1) > y
    c.vline(x, y, WH - y, rising ? "M" : "m")
    if (y < 19) c.vline(x, y, Math.min(5, 20 - y), rising ? "N" : "n")
  }
  c.rect(0, 36, WW, 12, "n")
})

// Rolling hills with round trees, the middle layer.
const tree = (c, x, base, size) => wrapX(x, (tx) => {
  c.rect(tx - 1, base - 4, 2, 5, "t")
  c.circ(tx, base - 5 - size, size, "G")
  c.circ(tx - 1, base - 6 - size, size - 1, "g")
  c.circ(tx - 2, base - 7 - size, Math.max(1, size - 3), "h")
})
scene("world_hills", { w: WW, h: WH }, (c) => {
  const top = (x) => Math.round(33 + wave(x, [[2, 2.5, 2.1], [4, 1.2, 0.5]]))
  for (let x = 0; x < WW; x++) c.vline(x, top(x), WH - top(x), "G")
  for (let x = 0; x < WW; x++) c.px(x, top(x), "g")
  ;[[10, 4], [31, 3], [52, 5], [78, 3], [101, 4], [123, 5], [146, 3]].forEach(([x, s]) => tree(c, x, top(x) + 1, s))
})

// Ground: grass edge and dirt path with stones, the fast front layer.
scene("world_ground", { w: WW, h: WH }, (c) => {
  c.rect(0, GROUND, WW, 2, "g")
  c.hline(0, GROUND, WW, "h")
  c.rect(0, GROUND + 2, WW, WH - GROUND - 2, "e")
  c.hline(0, GROUND + 2, WW, "E")
  for (let x = 0; x < WW; x += 8) {
    c.px(x + 3, GROUND - 1, "h")          // grass tufts above the edge
    if (x % 16 === 0) c.px(x + 5, GROUND - 1, "g")
  }
  for (let x = 0; x < WW; x += 20) {
    c.rect(x + 6, GROUND + 4, 3, 2, "r")   // stones
    c.hline(x + 6, GROUND + 5, 3, "R")
    c.px(x + 15, GROUND + 6, "d")          // pebbles
    c.hline(x + 1, GROUND + 3, 3, "d")     // worn path
  }
})

// Campsite props over the same world: a canvas tent and crossed logs.
scene("world_camp", { w: WW, h: WH }, (c) => {
  // Tent: A-frame side view, opening facing the fire.
  const x0 = 14, x1 = 50, apex = 22, base = GROUND + 1
  c.poly([x0, base, 30, apex, x1, base], "D")
  c.poly([30, apex, x1, base, 40, base], "d")          // shaded flank
  c.poly([33, base, 38, 30, 42, base], "E")            // opening
  c.poly([35, base, 38, 33, 40, base], "i")      // warm interior
  c.line(30, apex - 2, 30, apex, "t")                  // pole tip
  c.line(x0 - 2, base, x0 + 1, base - 1, "t")          // guy ropes
  c.line(x1 + 2, base, x1 - 1, base - 1, "t")
  // Stones ringing the fire spot.
  c.rect(60, GROUND, 14, 1, "R")
  c.px(60, GROUND - 1, "r")
  c.px(73, GROUND - 1, "r")
})

// Campfire: crossed logs and flickering flames with rising sparks.
anim("camp_fire", { w: 14, h: 18, frames: 6, fps: 8 },
  (c) => {
    const k = c.phase * TAU
    // Logs (behind the flame at the bottom).
    c.line(1, 17, 12, 13, "T", 2)
    c.line(1, 13, 12, 17, "t", 2)
    // Flame layers, swaying.
    const sway = Math.round(Math.sin(k) * 1.2)
    const lick = Math.round(Math.sin(k * 2 + 1) * 1.5)
    c.poly([2, 15, 6 + sway, 2 + Math.abs(lick), 12, 15], "F")
    c.poly([3, 15, 7 - sway, 5 + Math.abs(sway), 11, 15], "f")
    c.poly([5, 15, 7 + sway, 9, 9, 15], "y")
    c.px(7, 13, "v")
    // Sparks drifting up.
    const spark = (x, offset) => {
      const y = 7 - ((c.frame + offset) % 6) * 1.5
      if (y > 0) c.px(x, Math.round(y), "y")
    }
    spark(4, 0)
    spark(10, 3)
  },
  glow("fy", { r: 3, strength: 0.3 }))

// Composite previews (not shipped): the UI stacks the same layers.
include("hero.px.js")
scene("preview_trail", { w: WW, h: WH },
  stamp("world_sky", 0, 0), stamp("world_clouds", 0, 0), stamp("world_mountains", 0, 0),
  stamp("world_hills", 0, 0), stamp("world_ground", 0, 0), stamp("hero_walk", 30, GROUND - 27, { frame: 0 }))
scene("preview_camp", { w: WW, h: WH },
  stamp("world_sky", 0, 0), stamp("world_clouds", 0, 0), stamp("world_mountains", 0, 0),
  stamp("world_hills", 0, 0), stamp("world_ground", 0, 0), stamp("world_camp", 0, 0),
  stamp("hero_sit", 46, GROUND - 27, { frame: 0 }), stamp("camp_fire", 60, GROUND - 17, { frame: 1 }))
