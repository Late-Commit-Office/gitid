// Builds the gitid CLI and puts it where Tauri's `bundle > externalBin`
// expects it: src-tauri/binaries/gitid-<target triple>[.exe]. The installers
// then ship it next to the app (and on Linux in /usr/bin), so `gitid` works in
// a terminal without a separate download.
//
// Run by `beforeBuildCommand` / `beforeDevCommand` (tauri.conf.json), which
// set TAURI_ENV_TARGET_TRIPLE and TAURI_ENV_DEBUG. Run it by hand
// (`npm run sidecar`) before `cargo clippy -p gitid-app` or `cargo test -p
// gitid-app`: the app's build script fails while the file is missing.

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(app, "..");

const rustc = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
const host = /^host: (\S+)$/m.exec(rustc)?.[1];
if (!host) throw new Error("couldn't read the host target from `rustc -vV`");

const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
if (target === "universal-apple-darwin") {
  // Would need both Apple targets built and joined with `lipo`.
  throw new Error("build-sidecar: universal macOS builds aren't supported yet; build per architecture");
}
const debug = process.env.TAURI_ENV_DEBUG === "true" || process.argv.includes("--debug");
const profile = debug ? "debug" : "release";

const args = ["build", "-p", "gitid"];
if (!debug) args.push("--release");
// Only name the target when cross-building: with --target, cargo builds into
// target/<triple>/ and would compile everything a second time.
if (target !== host) args.push("--target", target);

console.log(`build-sidecar: cargo ${args.join(" ")}`);
execFileSync("cargo", args, { cwd: root, stdio: "inherit" });

const exe = target.includes("windows") ? ".exe" : "";
const targetDir = process.env.CARGO_TARGET_DIR ? resolve(root, process.env.CARGO_TARGET_DIR) : join(root, "target");
const built = join(targetDir, ...(target !== host ? [target] : []), profile, `gitid${exe}`);
const out = join(app, "src-tauri", "binaries", `gitid-${target}${exe}`);

mkdirSync(dirname(out), { recursive: true });
copyFileSync(built, out);
if (!exe) chmodSync(out, 0o755);
console.log(`build-sidecar: ${built} -> ${out}`);
