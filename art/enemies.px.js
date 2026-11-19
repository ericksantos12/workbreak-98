// WorkBreak 98 foes: the longer the focus block, the bigger the foe the mage faces.
// Original art. All foes face left (toward the hero) and stand on their bottom row.
//
// Build: node .claude/skills/pxl/bin/pxl.js build art/enemies.px.js --out art/out
// Then copy art/out/anims/{foe_*,spell_bolt,foe_poof}.png to src/assets/hero/.
//
// Tiers (current focus block): slime < 10 min, goblin 10-25, ogre 25-50, golem 50+.

pal({
  k: "#1b1630", w: "#efe7d2", y: "#ffd34e",
  t: "#6b4528", e: "#7b5131", E: "#4a2f1d",
  c: "#6ff0f0", C: "#2fa7c9", v: "#ffffff",
})
shades("GgH", "#58b54a", { spread: 0.4 })   // slime / goblin green: G dark, g mid, H light
shades("Uuj", "#8a6aa8", { spread: 0.4 })   // ogre purple
shades("Rrq", "#8d8a96", { spread: 0.45 })  // golem stone
shades("Mm", "#4f8a3a", { spread: 0.3 })    // moss
shades("Ss", "#9aa3b5", { spread: 0.4 })    // smoke

// Two-frame shuffle: body bobs, feet alternate.
const walk = (name, opts, draw) =>
  anim(name, { outline: "k", fps: 4, ...opts }, frame((c) => draw(c, 0)), frame((c) => draw(c, 1)))

// Tier 1: slime, a squishy hop.
walk("foe_slime", { w: 12, h: 9 }, (c, f) => {
  const ry = f ? 3 : 4, rx = f ? 6 : 5
  c.ellipse(6, 8 - ry, rx, ry, "g")
  c.ellipse(6, 8 - ry + 1, rx, ry - 1, "G")
  c.ellipse(6, 8 - ry - 1, rx - 1, ry - 1, "g")
  c.ellipse(5, 8 - ry * 2 + 2, 2, 1, "H")
  for (const x of [2, 5]) {   // eyes looking left: white with a dark pupil below
    c.px(x, 7 - ry, "w"), c.px(x + 1, 7 - ry, "w")
    c.px(x, 8 - ry, "k"), c.px(x + 1, 8 - ry, "w")
  }
})

// Tier 2: goblin with a club.
walk("foe_goblin", { w: 13, h: 15 }, (c, f) => {
  const b = f
  c.rect(3, 0 + b, 6, 1, "g")
  c.rect(2, 1 + b, 8, 5, "g")
  c.px(0, 2 + b, "G"), c.px(1, 2 + b, "g"), c.px(10, 2 + b, "g"), c.px(11, 2 + b, "G") // ears
  c.px(3, 2 + b, "y"), c.px(2, 2 + b, "k")                                            // eye
  c.hline(2, 4 + b, 2, "w")                                                            // teeth
  c.rect(3, 1 + b, 4, 1, "H")
  c.rect(3, 6 + b, 7, 4, "e")
  c.hline(3, 8 + b, 7, "E")
  c.px(2, 7 + b, "g")                                                                  // hand
  c.line(1, 7 + b, 0, 2 + b, "t", 2)                                                   // club
  const [l, r] = f ? [3, 7] : [4, 6]
  c.rect(l, 10 + b, 2, 5 - b, "G")
  c.rect(r, 10 + b, 2, 5 - b, "G")
})

// Tier 3: ogre with a raised club.
walk("foe_ogre", { w: 20, h: 22 }, (c, f) => {
  const b = f
  c.rect(f ? 6 : 7, 16, 3, 6, "U")
  c.rect(f ? 12 : 11, 16, 3, 6, "U")
  c.ellipse(10, 11 + b, 7, 6, "u")
  c.rect(5, 14 + b, 11, 3, "e")
  c.hline(5, 14 + b, 11, "E")
  c.circ(7, 5 + b, 4, "u")
  c.px(5, 4 + b, "y"), c.px(4, 4 + b, "k")
  c.px(4, 7 + b, "w"), c.px(7, 7 + b, "w")
  c.rect(2, 9 + b, 3, 6, "u")
  c.line(2, 12 + b, 1, 1 + b, "t", 3)
  c.ellipse(1, 2 + b, 2, 2, "E")
  c.shade([12, 0, 8, 22])
  c.light([3, 1, 6, 4])
})

// Tier 4: stone golem with glowing eyes and mossy shoulders.
walk("foe_golem", { w: 26, h: 28 }, (c, f) => {
  const b = f
  c.rect(f ? 7 : 8, 21, 5, 7, "R")
  c.rect(f ? 16 : 15, 21, 5, 7, "R")
  c.rect(5, 9 + b, 17, 13, "r")
  c.rect(6, 8 + b, 15, 1, "r")
  c.hline(6, 8 + b, 15, "M")
  c.hline(8, 9 + b, 4, "m")
  c.rect(7, 1 + b, 10, 8, "r")
  c.rect(8, 1 + b, 4, 2, "q")
  c.hline(7, 4 + b, 3, "k")
  c.px(8, 4 + b, "c"), c.px(9, 4 + b, "c")
  c.line(14, 12 + b, 17, 17 + b, "R")
  c.line(9, 14 + b, 11, 19 + b, "R")
  c.rect(1, 10 + b, 5, 9, "r")
  c.rect(0, 18 + b, 6, 5, "R")
  c.shade([17, 0, 9, 28])
}, )

// Magic bolt from the staff gem.
anim("spell_bolt", { w: 10, h: 6, fps: 10, frames: 2 }, (c) => {
  c.hline(0, 3, 4, c.frame ? "C" : "c")
  c.circ(7, 3, 2, "c")
  c.px(7, 2, "v")
}, glow("c", { r: 2, strength: 0.4 }))

// Defeat: a puff of smoke with a spark, four frames.
anim("foe_poof", { w: 18, h: 18, fps: 10, frames: 4 }, (c) => {
  const r = 3 + c.frame * 2
  const color = c.frame < 2 ? "q" : "S"
  c.circ(9, 10, r, color)
  c.circ(5, 12, Math.max(1, r - 2), "s")
  c.circ(13, 12, Math.max(1, r - 2), "s")
  if (c.frame < 3) {
    c.px(9, 9 - c.frame * 2, "y")
    c.px(4 - c.frame, 5, "y"), c.px(14 + c.frame, 5, "y")
  }
})

// Lineup preview (not shipped).
include("hero.px.js")
scene("preview_foes", { w: 120, h: 34, bg: "#a9dcf5" },
  rect(0, 31, 120, 3, "e"),
  stamp("hero_walk", 0, 4, { frame: 0 }),
  stamp("spell_bolt", 22, 14, { frame: 0 }),
  stamp("foe_slime", 34, 21, { frame: 0 }),
  stamp("foe_goblin", 48, 15, { frame: 0 }),
  stamp("foe_ogre", 64, 8, { frame: 0 }),
  stamp("foe_golem", 86, 1, { frame: 0 }),
  stamp("foe_poof", 102, 13, { frame: 1 }))
