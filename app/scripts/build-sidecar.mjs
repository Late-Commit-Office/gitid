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

let rustc;
try {
  rustc = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
} catch {
  console.error(
    "build-sidecar: can't run `rustc`. Install Rust from https://rustup.rs, then open a new terminal so it's on your PATH.",
  );
  process.exit(1);
}
const host = /^host: (\S+)$/m.exec(rustc)?.[1];
if (!host) throw new Error("couldn't read the host target from `rustc -vV`");

const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const debug = process.env.TAURI_ENV_DEBUG === "true" || process.argv.includes("--debug");
const profile = debug ? "debug" : "release";
const targetDir = process.env.CARGO_TARGET_DIR ? resolve(root, process.env.CARGO_TARGET_DIR) : join(root, "target");
const binaries = join(app, "src-tauri", "binaries");
mkdirSync(binaries, { recursive: true });

/** Build the CLI for one target and copy it to binaries/gitid-<triple>[.exe]. */
function buildFor(triple) {
  const args = ["build", "-p", "gitid"];
  if (!debug) args.push("--release");
  // Only name the target when cross-building: with --target, cargo builds into
  // target/<triple>/ and would compile everything a second time.
  if (triple !== host) args.push("--target", triple);

  console.log(`build-sidecar: cargo ${args.join(" ")}`);
  execFileSync("cargo", args, { cwd: root, stdio: "inherit" });

  const exe = triple.includes("windows") ? ".exe" : "";
  const built = join(targetDir, ...(triple !== host ? [triple] : []), profile, `gitid${exe}`);
  const out = join(binaries, `gitid-${triple}${exe}`);
  copyFileSync(built, out);
  if (!exe) chmodSync(out, 0o755);
  console.log(`build-sidecar: ${built} -> ${out}`);
  return out;
}

if (target === "universal-apple-darwin") {
  // `tauri build --target universal-apple-darwin` compiles the app once per
  // architecture (each looks for its own gitid-<arch>), then joins them and
  // ships gitid-universal-apple-darwin. So make all three.
  const parts = ["aarch64-apple-darwin", "x86_64-apple-darwin"].map(buildFor);
  const out = join(binaries, `gitid-${target}`);
  execFileSync("lipo", ["-create", "-output", out, ...parts], { stdio: "inherit" });
  console.log(`build-sidecar: lipo -> ${out}`);
} else {
  buildFor(target);
}
