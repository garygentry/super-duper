#!/usr/bin/env bun
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const FONT = "Segoe UI, Roboto, Helvetica, Arial, sans-serif";

const W = 920;
const H = 620;

// Layout
const cx = 360;          // center x of the main stack
const boxW = 420;
const boxH = 110;
const gapY = 70;         // vertical gap between layers (for arrows)
const topY = 90;

type Layer = {
  y: number;
  title: string;
  sub: string;
  tag: string;
  fill: string;
  stroke: string;
  grad: string;
  optional: boolean;
};

const layers: Layer[] = [
  {
    y: topY,
    title: "WinUI 3  (C# / .NET 10)",
    sub: "P/Invoke via EngineWrapper.cs",
    tag: "experimental · optional",
    fill: "#fef3c7",
    stroke: "#d97706",
    grad: "amberGrad",
    optional: true,
  },
  {
    y: topY + boxH + gapY,
    title: "super-duper-ffi  (cdylib)",
    sub: "Handle table · Callbacks",
    tag: "optional FFI boundary",
    fill: "#ede9fe",
    stroke: "#7c3aed",
    grad: "violetGrad",
    optional: true,
  },
  {
    y: topY + 2 * (boxH + gapY),
    title: "super-duper-core  (rlib)",
    sub: "Scanner · Hasher · Engine  ·  SQLite · RocksDB · Analysis",
    tag: "the engine — all business logic",
    fill: "#dbeafe",
    stroke: "#2563eb",
    grad: "blueGrad",
    optional: false,
  },
];

const left = cx - boxW / 2;

function grad(id: string, top: string, bottom: string) {
  return `<linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0%" stop-color="${top}"/>
    <stop offset="100%" stop-color="${bottom}"/>
  </linearGradient>`;
}

function layerBox(l: Layer, dashed: boolean) {
  const tx = cx;
  return `
  <g filter="url(#shadow)">
    <rect x="${left}" y="${l.y}" width="${boxW}" height="${boxH}" rx="12"
      fill="url(#${l.grad})" stroke="${l.stroke}" stroke-width="2"
      ${dashed ? 'stroke-dasharray="7 4"' : ""}/>
  </g>
  <text x="${tx}" y="${l.y + 42}" text-anchor="middle" font-family="${FONT}"
    font-size="19" font-weight="700" fill="#0f172a">${l.title}</text>
  <text x="${tx}" y="${l.y + 70}" text-anchor="middle" font-family="${FONT}"
    font-size="13" fill="#334155">${l.sub}</text>
  <text x="${tx}" y="${l.y + 93}" text-anchor="middle" font-family="${FONT}"
    font-size="11.5" font-weight="600" fill="${l.stroke}" letter-spacing="0.3">${l.tag}</text>`;
}

// Downward arrow between two boxes, with a label to the right of the line.
function downArrow(yTop: number, yBottom: number, label: string) {
  const x = cx;
  const y1 = yTop + boxH;
  const y2 = yBottom;
  return `
  <line x1="${x}" y1="${y1}" x2="${x}" y2="${y2 - 2}" stroke="#64748b"
    stroke-width="2.2" marker-end="url(#arrow)"/>
  <text x="${x + 16}" y="${(y1 + y2) / 2 + 4}" font-family="${FONT}"
    font-size="12" fill="#475569" font-style="italic">${label}</text>`;
}

// CLI node to the right, with an arrow into the core layer.
const cliX = left + boxW + 70;
const cliW = 150;
const cliH = 78;
const coreLayer = layers[2];
const cliY = coreLayer.y + (boxH - cliH) / 2;

const cliNode = `
  <g filter="url(#shadow)">
    <rect x="${cliX}" y="${cliY}" width="${cliW}" height="${cliH}" rx="12"
      fill="url(#greenGrad)" stroke="#16a34a" stroke-width="2"/>
  </g>
  <text x="${cliX + cliW / 2}" y="${cliY + 33}" text-anchor="middle" font-family="${FONT}"
    font-size="16" font-weight="700" fill="#0f172a">super-duper-cli</text>
  <text x="${cliX + cliW / 2}" y="${cliY + 55}" text-anchor="middle" font-family="${FONT}"
    font-size="11.5" font-weight="600" fill="#16a34a">primary entry point</text>`;

// Arrow: CLI -> core (links directly, solid emphatic).
const cliArrow = `
  <line x1="${cliX}" y1="${cliY + cliH / 2}" x2="${left + boxW + 3}" y2="${cliY + cliH / 2}"
    stroke="#16a34a" stroke-width="2.6" marker-end="url(#arrowGreen)"/>
  <text x="${cliX - 8}" y="${cliY + cliH / 2 - 12}" text-anchor="end" font-family="${FONT}"
    font-size="11" fill="#15803d" font-style="italic">links core crate directly</text>`;

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <filter id="shadow" x="-6%" y="-6%" width="112%" height="118%">
      <feDropShadow dx="2" dy="3" stdDeviation="3.5" flood-opacity="0.13"/>
    </filter>
    ${grad("amberGrad", "#fffbeb", "#fef3c7")}
    ${grad("violetGrad", "#f5f3ff", "#ede9fe")}
    ${grad("blueGrad", "#eff6ff", "#dbeafe")}
    ${grad("greenGrad", "#f0fdf4", "#dcfce7")}
    <marker id="arrow" markerWidth="11" markerHeight="8" refX="9.5" refY="4" orient="auto">
      <polygon points="0 0, 11 4, 0 8" fill="#64748b"/>
    </marker>
    <marker id="arrowGreen" markerWidth="11" markerHeight="8" refX="9.5" refY="4" orient="auto">
      <polygon points="0 0, 11 4, 0 8" fill="#16a34a"/>
    </marker>
  </defs>

  <rect x="0" y="0" width="${W}" height="${H}" rx="14" fill="#f8fafc" stroke="#e2e8f0" stroke-width="1.5"/>

  <text x="40" y="48" font-family="${FONT}" font-size="22" font-weight="800" fill="#0f172a">Super Duper — Architecture</text>
  <text x="40" y="70" font-family="${FONT}" font-size="12.5" fill="#64748b">Layered Cargo workspace: the core engine is consumed by the CLI directly, or via the optional FFI + UI path.</text>

  ${layerBox(layers[0], true)}
  ${downArrow(layers[0].y, layers[1].y, "C ABI  (u64 handles)")}
  ${layerBox(layers[1], true)}
  ${downArrow(layers[1].y, layers[2].y, "Rust function calls")}
  ${layerBox(layers[2], false)}

  ${cliArrow}
  ${cliNode}

  <!-- legend -->
  <g font-family="${FONT}" font-size="11.5" fill="#475569">
    <rect x="40" y="${H - 52}" width="13" height="13" rx="3" fill="#dbeafe" stroke="#2563eb" stroke-width="1.5"/>
    <text x="60" y="${H - 41}">Functional / actively developed</text>
    <rect x="270" y="${H - 52}" width="13" height="13" rx="3" fill="#fef3c7" stroke="#d97706" stroke-width="1.5" stroke-dasharray="4 2"/>
    <text x="290" y="${H - 41}">Experimental / optional</text>
    <rect x="490" y="${H - 52}" width="13" height="13" rx="3" fill="#dcfce7" stroke="#16a34a" stroke-width="1.5"/>
    <text x="510" y="${H - 41}">Headless CLI driver</text>
  </g>
</svg>
`;

const out = resolve(import.meta.dir, "..", "docs", "architecture.svg");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, svg);
console.log("wrote", out);
