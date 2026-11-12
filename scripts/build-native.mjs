/**
 * 构建 native cdylib → lib/<平台>.dylib。
 * ------------------------------------------------------------------
 * plugin.json 的 `native.enabled: true` 让宿主装载随包分发的本地库，约定路径：
 *
 *   macOS   → lib/macOS.dylib  （CI（Task 16）再 lipo universal2 + ad-hoc 签名）
 *   Windows → lib/Windows.dll
 *   Linux   → lib/Linux.so
 *
 * 本脚本只负责「本机目标」的构建与拷贝（dev 阶段够用）；交叉编译与
 * universal2 合并由 CI 的 native matrix 完成。
 *
 * 用法：node scripts/build-native.mjs
 */
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = path.join(repoRoot, 'native', 'Cargo.toml');
const libName = 'otools_fileshare_native'; // native/Cargo.toml 的 [lib] name

if (!fs.existsSync(manifest)) {
  console.error(`native/Cargo.toml 不存在：${manifest}`);
  process.exit(1);
}

console.log('native 模式：cargo build --release → lib/…');
// stdin 必须是 'ignore'：某些机器（安全软件挂钩匿名管道）上给子进程接 stdin 管道
// 会让 CreateProcess 直接 EBUSY（与 build-web.mjs 同因）。
const build = spawnSync('cargo', ['build', '--release', '--manifest-path', manifest], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
if (build.error) {
  console.error(`执行失败：cargo build --release（${build.error.message}）`);
  console.error('请先安装 Rust：https://rustup.rs');
  process.exit(1);
}
if ((build.status ?? 1) !== 0) {
  console.error('cargo build --release 失败，见上面的错误。');
  process.exit(build.status ?? 1);
}

// 平台产物命名（cargo 惯例）：macOS/Linux 带 lib 前缀，Windows 没有。
const targetDir = path.join(repoRoot, 'native', 'target', 'release');
const platform = process.platform;
const artifact =
  platform === 'darwin'
    ? path.join(targetDir, `lib${libName}.dylib`)
    : platform === 'win32'
      ? path.join(targetDir, `${libName}.dll`)
      : path.join(targetDir, `lib${libName}.so`);
const destName =
  platform === 'darwin' ? 'macOS.dylib' : platform === 'win32' ? 'Windows.dll' : 'Linux.so';

if (!fs.existsSync(artifact)) {
  console.error(`构建产物不存在：${artifact}`);
  process.exit(1);
}

const libDir = path.join(repoRoot, 'lib');
fs.mkdirSync(libDir, { recursive: true });
const dest = path.join(libDir, destName);
fs.copyFileSync(artifact, dest);

const size = (fs.statSync(dest).size / 1048576).toFixed(1);
console.log(`已拷贝 ${artifact} → lib/${destName}（${size} MB）`);
console.log(`\n完成。宿主 quickDev / 打包（pack-plugin.mjs 的 meta.nativeLibs）都从 lib/${destName} 取本地库。`);
