// Read-only attribution of the exact bindgen artifacts used by compare.cjs.
// node benchmarks/analyze-display-size.cjs OUT.json before=SITE glory=SITE dioxus=SITE
'use strict';
const fs = require('node:fs'), path = require('node:path'), cp = require('node:child_process');
const crypto = require('node:crypto'), zlib = require('node:zlib');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sizes = bytes => ({bytes: bytes.length, gzip: zlib.gzipSync(bytes, {level: 9}).length,
  brotli: zlib.brotliCompressSync(bytes, {params: {[zlib.constants.BROTLI_PARAM_QUALITY]: 11}}).length});
const output = {};
for (const arg of process.argv.slice(3)) {
  const at = arg.indexOf('=');
  if (at < 1) throw Error('Expected label=site');
  const label = arg.slice(0, at), site = path.resolve(arg.slice(at + 1));
  const file = path.join(site, 'app_bg.wasm'), wasm = fs.readFileSync(file);
  const attribution = JSON.parse(cp.execFileSync(process.execPath,
    [path.join(__dirname, 'wasm-size/attribute.cjs'), file, file], {encoding: 'utf8', maxBuffer: 8 * 1024 * 1024}));
  // Diagnostic metadata stripping only; timed artifacts are never rewritten.
  const executable = [wasm.subarray(0, 8)]; let offset = 8;
  while (offset < wasm.length) {
    const start = offset, id = wasm[offset++]; let size = 0, shift = 0, byte;
    do { byte = wasm[offset++]; size += (byte & 127) * 2 ** shift; shift += 7; } while (byte & 128);
    offset += size;
    if (id) executable.push(wasm.subarray(start, offset));
  }
  const seen = new Set(), javascript = [];
  function visit(file) {
    if (seen.has(file)) return;
    if (!file.startsWith(site + path.sep)) throw Error('Import escaped site');
    seen.add(file);
    const bytes = fs.readFileSync(file), source = bytes.toString('utf8');
    javascript.push({path: path.relative(site, file).replaceAll('\\', '/'), sha256: sha(bytes), ...sizes(bytes)});
    for (const match of source.matchAll(/\b(?:from\s*|import\s*)['"](\.[^'"]+\.js)['"]/g))
      visit(path.resolve(path.dirname(file), match[1]));
  }
  visit(path.join(site, 'app.js'));
  const emitted = fs.readdirSync(site, {recursive: true}).filter(f => f.endsWith('.js') && f !== 'bench-harness.js');
  output[label] = {site, wasm: {...sizes(wasm), sha256: sha(wasm)},
    withoutCustomSections: sizes(Buffer.concat(executable)),
    javascript: {files: javascript, ...Object.fromEntries(['bytes', 'gzip', 'brotli'].map(key =>
      [key, javascript.reduce((n, file) => n + file[key], 0)])),
      emittedBytes: emitted.reduce((n, file) => n + fs.statSync(path.join(site, file)).size, 0),
      unreferenced: emitted.filter(file => !seen.has(path.resolve(site, file)))}, attribution};
}
fs.writeFileSync(process.argv[2], JSON.stringify(output, null, 2) + '\n');
for (const [label, result] of Object.entries(output))
  console.log(label, JSON.stringify({wasm: result.wasm, executable: result.withoutCustomSections,
    javascript: {bytes: result.javascript.bytes, emittedBytes: result.javascript.emittedBytes, brotli: result.javascript.brotli}}));
