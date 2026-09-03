#!/usr/bin/env node

const { spawnSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const platform = os.platform();
const binName = platform === 'win32' ? 'flareperf.exe' : 'flareperf';
const localBinPath = path.join(__dirname, binName);

let targetBin = localBinPath;

if (!fs.existsSync(localBinPath)) {
  // Check if available globally in PATH
  try {
    const check = spawnSync(binName, ['--version'], { stdio: 'ignore' });
    if (check.status === 0) {
      targetBin = binName;
    } else {
      console.error(`Error: Could not find flareperf binary at ${localBinPath}`);
      console.error('Run npm rebuild flareperf or install via cargo: cargo install flareperf');
      process.exit(1);
    }
  } catch (_e) {
    console.error(`Error: Could not find flareperf binary at ${localBinPath}`);
    console.error('Run npm rebuild flareperf or install via cargo: cargo install flareperf');
    process.exit(1);
  }
}

const args = process.argv.slice(2);
const result = spawnSync(targetBin, args, { stdio: 'inherit' });

if (result.error) {
  console.error('Failed to execute flareperf:', result.error);
  process.exit(1);
}

process.exit(result.status ?? 0);
