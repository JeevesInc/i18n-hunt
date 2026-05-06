const fs = require("node:fs");
const path = require("node:path");

function getBinaryName() {
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "linux" && arch === "x64") return "i18n-hunt-linux-x64";
    if (platform === "darwin" && arch === "x64") return "i18n-hunt-darwin-x64";
    if (platform === "darwin" && arch === "arm64") return "i18n-hunt-darwin-arm64";
    if (platform === "win32" && arch === "x64") return "i18n-hunt-win32-x64.exe";

    throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

const rootDir = path.resolve(__dirname, "..", "..");
const sourceBinary =
    process.platform === "win32"
        ? path.join(rootDir, "target", "release", "i18n-hunt.exe")
        : path.join(rootDir, "target", "release", "i18n-hunt");

const nativeDir = path.join(rootDir, "npm", "native");
const targetBinary = path.join(nativeDir, getBinaryName());

fs.mkdirSync(nativeDir, { recursive: true });
fs.copyFileSync(sourceBinary, targetBinary);

if (process.platform !== "win32") {
    fs.chmodSync(targetBinary, 0o755);
}

console.log(`Copied ${sourceBinary} -> ${targetBinary}`);
