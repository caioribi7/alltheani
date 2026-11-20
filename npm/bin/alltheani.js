#!/usr/bin/env node
// Baixa o binário do alltheani da release do GitHub (uma vez), confere o SHA256 e roda.
"use strict";
const { spawnSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const https = require("node:https");
const path = require("node:path");

const REPO = "caioribi7/alltheani";
const VERSION = require("../package.json").version;

const ASSETS = {
  "linux-x64": "alltheani-linux-x86_64",
  "darwin-arm64": "alltheani-macos-arm64",
  "darwin-x64": "alltheani-macos-x86_64",
  "win32-x64": "alltheani-windows-x86_64.exe",
};

function get(url, redirects = 5) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "alltheani-npm" } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location && redirects > 0) {
          res.resume();
          return resolve(get(res.headers.location, redirects - 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} em ${url}`));
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
      })
      .on("error", reject);
  });
}

async function ensureBinary() {
  const asset = ASSETS[`${process.platform}-${process.arch}`];
  if (!asset) {
    throw new Error(`Sem binário para ${process.platform}-${process.arch}. Instale com: cargo install alltheani`);
  }
  const dir = path.join(__dirname, ".cache", VERSION);
  const bin = path.join(dir, process.platform === "win32" ? "alltheani.exe" : "alltheani");
  if (fs.existsSync(bin)) return bin;

  const base = `https://github.com/${REPO}/releases/download/v${VERSION}`;
  process.stderr.write(`› Baixando ${asset} (v${VERSION})…\n`);
  const [data, sums] = await Promise.all([get(`${base}/${asset}`), get(`${base}/SHA256SUMS`)]);
  const line = sums.toString().split("\n").find((l) => l.trim().endsWith(` ${asset}`));
  if (!line) throw new Error(`Checksum de ${asset} não encontrado.`);
  const expected = line.trim().split(/\s+/)[0].toLowerCase();
  const actual = crypto.createHash("sha256").update(data).digest("hex");
  if (expected !== actual) throw new Error("O arquivo baixado não confere com o checksum. Nada foi instalado.");

  fs.mkdirSync(dir, { recursive: true });
  const tmp = `${bin}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, data, { mode: 0o755 });
  fs.renameSync(tmp, bin);
  return bin;
}

ensureBinary()
  .then((bin) => {
    const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
    process.exit(r.status ?? 1);
  })
  .catch((e) => {
    process.stderr.write(`✗ ${e.message}\n`);
    process.exit(1);
  });
