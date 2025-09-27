import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');
const tauriModuleDir = path.join(projectRoot, 'src-tauri/modules/app');
const tauriCliEntry = path.join(projectRoot, 'node_modules/@tauri-apps/cli/tauri.js');
const overrideConfig = path.join(projectRoot, 'scripts/tauri.local.json');

const [, , command, ...restArgs] = process.argv;

if (!command) {
  console.error('Usage: node scripts/tauri-runner.mjs <command> [args...]');
  process.exit(1);
}

const child = spawn(
  process.execPath,
  [tauriCliEntry, command, '--config', overrideConfig, ...restArgs],
  {
    cwd: tauriModuleDir,
    stdio: 'inherit',
    env: {
      ...process.env,
    },
  }
);

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 0);
  }
});
