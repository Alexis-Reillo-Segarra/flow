// Rasteriza los SVG de marca a PNG y ensambla flow.ico.
//
//   npm install @resvg/resvg-js   (fuera del repo: esto es un proyecto Rust)
//   node render.js
//
// Los SVG son la fuente; todo lo que genera este script es derivado.

const fs = require('fs');
const path = require('path');
const { Resvg } = require('@resvg/resvg-js');

const BRAND = __dirname;
const OUT = path.join(BRAND, 'png');
fs.mkdirSync(OUT, { recursive: true });

const render = (svgFile, width) => {
  const svg = fs.readFileSync(path.join(BRAND, svgFile), 'utf8');
  return new Resvg(svg, { fitTo: { mode: 'width', value: width } }).render().asPng();
};

const jobs = [
  ['flow-icon-dark.svg', [1024, 512, 256, 128, 64]],
  ['flow-icon-light.svg', [1024, 512, 256, 128, 64]],
  ['flow-mark-dark.svg', [512]],
  ['flow-mark-light.svg', [512]],
  ['flow-favicon.svg', [16, 32, 48]],
];

for (const [file, widths] of jobs) {
  for (const w of widths) {
    const png = render(file, w);
    const name = `${path.basename(file, '.svg')}-${w}.png`;
    fs.writeFileSync(path.join(OUT, name), png);
    console.log(`${name}  ${png.length} bytes`);
  }
}

// ICO: los tamaños pequeños salen de la versión afinada para tamaños pequeños,
// el de 256 del icono grande.
const parts = [
  [16, render('flow-favicon.svg', 16)],
  [32, render('flow-favicon.svg', 32)],
  [48, render('flow-favicon.svg', 48)],
  [256, render('flow-icon-dark.svg', 256)],
];

const dir = Buffer.alloc(6);
dir.writeUInt16LE(0, 0); // reservado
dir.writeUInt16LE(1, 2); // tipo: icono
dir.writeUInt16LE(parts.length, 4);

let offset = 6 + 16 * parts.length;
const entries = [];
for (const [size, png] of parts) {
  const e = Buffer.alloc(16);
  const dim = size === 256 ? 0 : size; // 256 se codifica como 0
  e.writeUInt8(dim, 0);
  e.writeUInt8(dim, 1);
  e.writeUInt8(0, 2); // paleta
  e.writeUInt8(0, 3); // reservado
  e.writeUInt16LE(1, 4); // planos
  e.writeUInt16LE(32, 6); // bits por píxel
  e.writeUInt32LE(png.length, 8);
  e.writeUInt32LE(offset, 12);
  entries.push(e);
  offset += png.length;
}

const ico = Buffer.concat([dir, ...entries, ...parts.map(([, p]) => p)]);
fs.writeFileSync(path.join(BRAND, 'flow.ico'), ico);
console.log(`flow.ico  ${ico.length} bytes  (${parts.map(([s]) => s).join('/')})`);
