// Rebuild from installed, locked dependencies. Includes Windows build-time
// crates conservatively; never substitutes a licence inferred from its name.
import { readFile, readdir, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cargo = process.env.CARGO ?? (process.platform === "win32" ? path.join(process.env.USERPROFILE, ".cargo/bin/cargo.exe") : "cargo");
const metadata = JSON.parse(execFileSync(cargo, ["metadata", "--locked", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc", "--manifest-path", path.join(root, "app/src-tauri/Cargo.toml")], {encoding: "utf8", maxBuffer: 32 * 1024 * 1024, windowsHide: true}));
const reachable = new Set(metadata.resolve.nodes.map(n => n.id));
const packages = metadata.packages.filter(p => reachable.has(p.id) && p.source).map(p => ({name: p.name, version: p.version, license: p.license, directory: path.dirname(p.manifest_path), licenseFile: p.license_file, source: p.repository ?? p.source}));
const npmLock = JSON.parse(await readFile(path.join(root, "app/package-lock.json"), "utf8"));
for (const [location, item] of Object.entries(npmLock.packages)) {
  if (!location || item.dev) continue;
  const directory = path.join(root, "app", location);
  const p = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8"));
  packages.push({name: p.name, version: p.version, license: p.license, directory, source: typeof p.repository === "string" ? p.repository : p.repository?.url});
}
packages.sort((a,b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`));
const missing = [];
const texts = new Map();
function licenceReference(value) {
  const text = value.replace(/[ \t]+$/gm, "").trim();
  if (!texts.has(text)) texts.set(text, `L${String(texts.size + 1).padStart(3, "0")}`);
  const id = texts.get(text);
  return `[${id}](#${id.toLowerCase()})`;
}
const supplements = {
  "alloc-stdlib@0.3.0": "alloc-stdlib.txt", "tauri-plugin@2.7.2": "tauri-plugin.txt",
  "webview2-com@0.39.1": "webview2.txt", "webview2-com-sys@0.39.1": "webview2.txt",
  "webview2-com-macros@0.8.1": "webview2.txt", "selectors@0.38.0": "MPL-2.0.txt",
};
let output = "# Third-party notices\n\nGenerated from the locked Windows dependency graph and production npm packages. Build-time crates are included conservatively. GameZip is MIT-licensed; bundled dependencies retain their own licences below.\n\nFFmpeg is not distributed with GameZip. WebView2 is a separately installed Microsoft runtime. Reference mockups are not shipped as app assets.\n\n";
for (const p of packages) {
  const names = (await readdir(p.directory, {withFileTypes: true})).filter(e => e.isFile() && /^(licen[cs]e|copying|notice|copyright)([.\-_]|$)/i.test(e.name)).map(e => e.name);
  if (p.licenseFile && !names.includes(p.licenseFile)) names.push(p.licenseFile);
  output += `## ${p.name} ${p.version}\n\nDeclared licence: ${p.license ?? "not declared"}\n\nSource: ${p.source ?? "package registry"}\n\n`;
  if (!names.length) {
    const supplement = supplements[`${p.name}@${p.version}`];
    if (supplement) output += `Upstream licence: ${licenceReference(await readFile(path.join(root, "docs/licenses", supplement), "utf8"))}\n\n`;
    else {missing.push(`${p.name}@${p.version}`); output += "Licence text is not included in this package; distribution review required.\n\n";}
  }
  if (p.name === "selectors") output += `Unmodified corresponding source: https://crates.io/api/v1/crates/selectors/${p.version}/download and https://github.com/servo/stylo/tree/572ecba2d1600e7c3d490586692a209faf703baa/selectors\n\n`;
  for (const name of names.sort()) output += `${name}: ${licenceReference(await readFile(path.join(p.directory, name), "utf8"))}\n\n`;
}
for (const [text, id] of texts) output += `## ${id}\n\n${text}\n\n`;
await writeFile(path.join(root, "THIRD_PARTY_NOTICES.md"), output.trimEnd() + "\n");
console.log(`Collected notices for ${packages.length} packages. Missing licence text: ${missing.length}.`);
if (missing.length) console.log(missing.join("\n"));
if (missing.length) process.exitCode = 1;
