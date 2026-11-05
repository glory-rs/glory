// Resolve verification output within the owning Cargo target directory.
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
let target;
function localBuildPath(suffix = '') {
  if (!target) {
    target = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'],
      { cwd: root, encoding: 'utf8' })).target_directory;
  }
  return path.join(target, 'verification', suffix);
}
module.exports = { localBuildPath };
