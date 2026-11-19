// WorkBreak 98 hero: a small wandering mage (original art, loosely inspired by classic
// blue-hat wizard sprites). Source for the progression panel sprites.
//
// Build: node .claude/skills/pxl/bin/pxl.js build art/hero.px.js --out art/out
// Then copy art/out/anims/hero_*.png to src/assets/hero/ (art/out is git-ignored).
//
// Every anim uses the same canvas (22x26 + 1px outline = 24x28 per frame) and the feet
// rest on row 26, so the UI can swap poses without moving the hero. Faces right; light
// comes from the top-left.

pal({ k: "#1b1630", w: "#efe7d2", W: "#b8ae98", y: "#f2c14e", Y: "#b9862c", H: "#3a3550", g: "#6ff0f0" })
shades("abc", "#3d63c9", { spread: 0.45 })   // hat and robe: a dark, b mid, c light
shades("pq", "#d9926a", { spread: 0.18, shift: 4 })   // skin
shades("Tt", "#8a5a2b", { spread: 0.3 })   // staff (T dark, t light)
shades("Ll", "#4a3226", { spread: 0.3 })   // boots

const W = 22, H = 26

// Upper body and robe, 14x17. Hat tip curls back (left); staff in the front hand.
sprite("_body", `
  .cc...........
  ..cbb.........
  ...bbbb.......
  ...bbbbb......
  ..bbbbbbb.....
  ..yyyyyyyY...g
  aabbbbbbbbba.g
  ...Hqqqkq...tt
  ...Hqqqqqp..t.
  ....pqqqp...t.
  ..wwbbbbbb..t.
  .wwbbbbbbaqqt.
  .wwbbbbbba..t.
  .wWyyyyyyY..t.
  .wWbbbbbba..T.
  ..Wbbbbbbba.T.
  ..aaaaaaaaa.T.`)

sprite("_feet_stand", "....LL.LL...")
sprite("_feet_stride", "..LL....LL..")
sprite("_feet_stride2", ".LL.....LL..")
sprite("_feet_pass", ".....LLL....")
sprite("_hem_sit", ".aaaaaaaaaaa.")

const BX = 3, BY = 8 // body origin: body rows 8..24, feet on row 25
const shut = (x, y) => grid(x + 7, y + 7, "p")
const body = (dy = 0) => stamp("_body", BX, BY + dy)
const feet = (name, dy = 0) => stamp(name, BX, BY + 17 + dy)

// Standing still, blinking now and then.
anim("hero_idle", { w: W, h: H, outline: "k" },
  frame({ ms: 1800 }, body(), feet("_feet_stand")),
  frame({ ms: 160 }, body(), feet("_feet_stand"), shut(BX, BY)))

// Walking: stride and pass, the body bobbing one pixel on the pass.
anim("hero_walk", { w: W, h: H, outline: "k", fps: 8 },
  frame(body(), feet("_feet_stride")),
  frame(body(-1), feet("_feet_pass")),
  frame(body(), feet("_feet_stride2")),
  frame(body(-1), feet("_feet_pass")))

// Sitting: the robe pools on the ground (one row lower, no feet).
const sit = () => [stamp("_body", BX, BY + 1), stamp("_hem_sit", BX + 1, BY + 17)]

// Sitting by the fire, warming up, blinking slowly.
anim("hero_sit", { w: W, h: H, outline: "k" },
  frame({ ms: 2400 }, ...sit()),
  frame({ ms: 200 }, ...sit(), shut(BX, BY + 1)))

// Dozing: sitting, eyes shut, a "z" drifting up.
const z = (x, y, big) => grid(x, y, big ? "wwww\n..w.\n.w..\nwwww" : "ww.\n.w.\n.ww")
anim("hero_sleep", { w: W, h: H, outline: "k", fps: 2 },
  frame(...sit(), shut(BX, BY + 1), z(14, 7, false)),
  frame(...sit(), shut(BX, BY + 1), z(15, 3, true)),
  frame(...sit(), shut(BX, BY + 1), z(17, 0, true)))
