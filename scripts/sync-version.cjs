#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const root = process.cwd();
const pkgPath = path.join(root, "package.json");
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");
const cargoPath = path.join(root, "src-tauri", "Cargo.toml");

const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
const version = pkg.version;

// Update tauri.conf.json top-level version
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf8"));
tauriConf.version = version;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");

// Update Cargo.toml [package] version first occurrence
let cargoToml = fs.readFileSync(cargoPath, "utf8");
cargoToml = cargoToml.replace(/^(version\s*=\s*)"[^"]*"/m, `$1"${version}"`);
fs.writeFileSync(cargoPath, cargoToml);

console.log(`Synced version ${version} to tauri.conf.json and Cargo.toml`);
