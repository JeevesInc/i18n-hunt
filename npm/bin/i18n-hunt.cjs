#!/usr/bin/env node

const path = require("node:path");
const { spawnSync } = require("node:child_process");

function getBinaryName() {
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "linux" && arch === "x64") {
        return "i18n-hunt-linux-x64";
    }

    if (platform === "darwin" && arch === "x64") {
        return "i18n-hunt-darwin-x64";
    }

    if (platform === "darwin" && arch === "arm64") {
        return "i18n-hunt-darwin-arm64";
    }

    if (platform === "win32" && arch === "x64") {
        return "i18n-hunt-win32-x64.exe";
    }

    throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

const binary = path.join(__dirname, "..", "native", getBinaryName());

const result = spawnSync(binary, process.argv.slice(2), {
    stdio: "inherit",
});

process.exit(result.status ?? 1);
