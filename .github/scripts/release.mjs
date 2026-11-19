import { readFileSync, writeFileSync, appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function parseChangelog(text) {
  const headings = [...text.matchAll(/^##\s+\[([^\]]+)\].*$/gm)];
  const index = headings.findIndex((match) => match[1].toLowerCase() !== "unreleased");
  if (index < 0) throw new Error("CHANGELOG.md needs a version section such as ## [0.1.0]");
  const heading = headings[index];
  const version = heading[1];
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error("The latest changelog version must be MAJOR.MINOR.PATCH");
  }
  const notes = text.slice(heading.index + heading[0].length, headings[index + 1]?.index).trim();
  if (!notes) throw new Error("The latest changelog version needs release notes");
  return { version, tag: `v${version}`, notes };
}

export function syncVersion(version) {
  for (const path of ["package.json", "package-lock.json", "src-tauri/tauri.conf.json"]) {
    const json = JSON.parse(readFileSync(path, "utf8"));
    json.version = version;
    if (path === "package-lock.json") json.packages[""].version = version;
    writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
  }
  for (const [path, pattern] of [
    ["src-tauri/Cargo.toml", /(\[package\][\s\S]*?\nversion = ")[^"]+(")/],
    ["src-tauri/Cargo.lock", /(\[\[package\]\]\r?\nname = "workbreak-98"\r?\nversion = ")[^"]+(")/],
  ]) {
    const text = readFileSync(path, "utf8");
    if (!pattern.test(text)) throw new Error(`Cannot find application version in ${path}`);
    writeFileSync(path, text.replace(pattern, (_, prefix, suffix) => prefix + version + suffix));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { version, tag, notes } = parseChangelog(readFileSync("CHANGELOG.md", "utf8"));
  writeFileSync("release-notes.md", notes + "\n");
  if (process.argv.includes("--sync")) syncVersion(version);
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\ntag=${tag}\n`);
  }
  console.log(tag);
}
