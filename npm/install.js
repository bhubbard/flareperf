const fs = require('fs');
const path = require('path');
const https = require('https');
const os = require('os');
const { execSync } = require('child_process');

const platform = os.platform();
const arch = os.arch();

const REPO = 'bhubbard/flareperf';
const VERSION = require('./package.json').version;

let assetName = 'flareperf';

if (platform === 'win32') {
  if (arch !== 'x64') throw new Error('Unsupported architecture on Windows: ' + arch);
  assetName = 'flareperf-win32-x64.exe';
} else if (platform === 'darwin') {
  if (arch === 'x64') assetName = 'flareperf-darwin-x64';
  else if (arch === 'arm64') assetName = 'flareperf-darwin-arm64';
  else throw new Error('Unsupported architecture on macOS: ' + arch);
} else if (platform === 'linux') {
  if (arch !== 'x64') throw new Error('Unsupported architecture on Linux: ' + arch);
  assetName = 'flareperf-linux-x64';
} else {
  throw new Error('Unsupported platform: ' + platform);
}

const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${assetName}`;

const binDir = path.join(__dirname, 'bin');
const binaryPath = path.join(binDir, platform === 'win32' ? 'flareperf.exe' : 'flareperf');

if (!fs.existsSync(binDir)) {
  fs.mkdirSync(binDir, { recursive: true });
}

// If binary is already present, skip download
if (fs.existsSync(binaryPath)) {
  console.log(`flareperf binary already present at ${binaryPath}`);
  process.exit(0);
}

console.log(`Downloading flareperf v${VERSION} for ${platform} ${arch}...`);
console.log(`URL: ${url}`);

function download(downloadUrl, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https.get(downloadUrl, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        download(response.headers.location, dest).then(resolve).catch(reject);
      } else if (response.statusCode === 200) {
        response.pipe(file);
        file.on('finish', () => {
          file.close();
          resolve();
        });
      } else {
        reject(new Error(`Failed to download: ${response.statusCode} ${response.statusMessage}`));
      }
    }).on('error', (err) => {
      fs.unlink(dest, () => reject(err));
    });
  });
}

download(url, binaryPath)
  .then(() => {
    if (platform !== 'win32') {
      execSync(`chmod +x "${binaryPath}"`);
    }
    console.log('Successfully installed flareperf binary.');
  })
  .catch((err) => {
    console.warn('Notice: Could not automatically download prebuilt binary:', err.message);
    console.warn('You can build flareperf from source with: cargo install flareperf');
  });
