import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, copyFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseChangelog, syncVersion } from "./release.mjs";

test("uses only the newest numbered release, excluding unreleased and older notes", () => {
  assert.deepEqual(parseChangelog(
    "# Changelog\n## [Unreleased]\nNot ready\n## [0.2.0] - 2026-11-03\n### Added\n- Timer\n## [0.1.0]\nOlder",
  ), { version: "0.2.0", tag: "v0.2.0", notes: "### Added\n- Timer" });
});

test("rejects missing, invalid or empty release sections", () => {
  for (const text of ["# Changelog", "## [Unreleased]\nPending", "## [01.2.0]\nNotes", "## [0.2.0]\n\n## [0.1.0]\nOld"]) {
    assert.throws(() => parseChangelog(text));
  }
});

test("syncs npm, Cargo and Tauri versions without changing dependency versions", () => {
  const original = process.cwd();
  const directory = mkdtempSync(join(tmpdir(), "workbreak-release-"));
  try {
    mkdirSync(join(directory, "src-tauri"));
    const paths = ["package.json", "package-lock.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];
    for (const path of paths) copyFileSync(join(original, path), join(directory, path));
    const before = JSON.parse(readFileSync(join(original, "package.json"), "utf8"));
    process.chdir(directory);
    syncVersion("9.8.7");
    for (const path of paths.slice(0, 3)) {
      assert.equal(JSON.parse(readFileSync(path, "utf8")).version, "9.8.7");
    }
    assert.equal(JSON.parse(readFileSync("package-lock.json", "utf8")).packages[""].version, "9.8.7");
    assert.deepEqual(JSON.parse(readFileSync("package.json", "utf8")).dependencies, before.dependencies);
    assert.match(readFileSync("src-tauri/Cargo.toml", "utf8"), /\[package\][\s\S]*?version = "9.8.7"/);
    const lock = readFileSync("src-tauri/Cargo.lock", "utf8");
    const oldLock = readFileSync(join(original, "src-tauri/Cargo.lock"), "utf8");
    assert.match(lock, /name = "workbreak-98"\r?\nversion = "9.8.7"/);
    assert.equal(lock.replace('version = "9.8.7"', 'version = "ORIGINAL"'),
      oldLock.replace(/(name = "workbreak-98"\r?\n)version = "[^"]+"/, '$1version = "ORIGINAL"'));
  } finally {
    process.chdir(original);
    rmSync(directory, { recursive: true, force: true });
  }
});
