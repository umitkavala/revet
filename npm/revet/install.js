"use strict";

const os = require("os");

const PLATFORMS = {
  "darwin-arm64": "@revet/cli-darwin-arm64",
  "darwin-x64": "@revet/cli-darwin-x64",
  "linux-arm64": "@revet/cli-linux-arm64",
  "linux-x64": "@revet/cli-linux-x64",
  "win32-x64": "@revet/cli-win32-x64",
  "win32-arm64": "@revet/cli-win32-arm64",
};

const key = `${os.platform()}-${os.arch()}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  console.warn(
    `warn: revet does not have a prebuilt binary for ${key}.\n` +
      `You can build from source: cargo install revet`
  );
  process.exit(0);
}

try {
  const binName = os.platform() === "win32" ? "revet.exe" : "revet";
  require.resolve(`${pkg}/bin/${binName}`);
} catch {
  console.warn(
    `warn: revet platform package ${pkg} not found.\n` +
      `This may happen if npm skipped optional dependencies.\n` +
      `You can install it manually: npm install ${pkg}\n` +
      `Or build from source: cargo install revet`
  );
}
