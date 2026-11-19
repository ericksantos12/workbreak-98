// Before / after: the same mushroom drawn the common way and with the fork's tools.
//   before: flat ramp from the palette, black outline everywhere, pillow shading (light in the middle)
//   after:  shades() hue-shifted ramps, selout (outline takes the neighbor's darker shade),
//           one light direction (top-left), and `pxl lint` clean
use("pico8")
pal({ k: "#1a1220", w: "#fff4e0", r: "#b8323c", R: "#e05a5a", L: "#ff9a8a", s: "#b8a888", S: "#e0d4b4", z: "#22182a" })

sprite("before", { w: 24, h: 24, outline: "k" },
  ellipse(12, 9, 10, 7, "r"), ellipse(12, 9, 7, 5, "R"), ellipse(12, 9, 4, 3, "L"),
  rect(9, 15, 6, 8, "s"), rect(10, 16, 4, 6, "S"),
  circ(7, 8, 1, "w"), circ(16, 6, 1, "w"), circ(14, 11, 1, "w"))

shades("1234", "#c8323c")          // cap: cool dark red -> warm pink-orange
shades("5678", "#d8c8a0")          // stem
pal({ w: "#fff4e0" })

sprite("after", { w: 24, h: 24, selout: "z" },
  ellipse(12, 9, 10, 7, "3"),
  shade((x, y) => Math.hypot(x - 7, (y - 4) * 1.3) > 8),            // falls off away from the light
  shade((x, y) => Math.hypot(x - 7, (y - 4) * 1.3) > 12.5),
  ellipse(8, 5, 3, 1, "4"), px(5, 6, "4"),                           // glint, top-left
  clip([0, 13, 24, 13, 24, 17, 0, 17], ellipse(12, 9, 10, 7, "1")),  // cap underside in shadow
  rect(9, 15, 6, 8, "6"), vline(10, 16, 6, "7"), vline(14, 15, 8, "5"), hline(9, 15, 6, "5"),
  circ(7, 8, 1, "w"), px(16, 5, "w"), px(17, 5, "8"), px(16, 6, "8"), circ(15, 11, 1, "7"), px(14, 10, "8"))

scene("polish", { w: 64, h: 34, bg: "#2b2235" },
  stamp("before", 4, 2), stamp("after", 36, 2),
  text(5, 28, "BEFORE", "S"), text(40, 28, "AFTER", "8"))
