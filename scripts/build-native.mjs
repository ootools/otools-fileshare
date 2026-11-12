/**
 * 构建 native cdylib → lib/<平台><架构>.dylib（+ 本机架构的宿主默认名副本）。
 * ------------------------------------------------------------------
 * plugin.json 的 `native.enabled: true` 让宿主装载随包分发的本地库，约定路径：
 *
 *   macOS   → lib/macOS.dylib  （CI 里由 arm64 + x64 两份 lipo 出 universal2）
 *   Windows → lib/Windows.dll
 *   Linux   → lib/Linux.so
 *
 * --arch x64|arm64（默认本机架构）：交叉编译对应架构，并额外产出带架构
 * 后缀的副本（lib/macOS-arm64.dylib / lib/Windows-x64.dll / lib/Linux-arm64.so …），
 * 供 pack-plugin.mjs --platform/--arch 打单架构包使用；本机架构额外产出
 * 宿主默认名副本（本地 quickDev 装载 / 合包兜底）。
 *
 * 用法：node scripts/build-native.mjs [--arch x64|arm64]
 */
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = path.join(repoRoot, 'native', 'Cargo.toml');
const libName = 'otools_fileshare_native'; // native/Cargo.toml 的 [lib] name

const archIndex = process.argv.indexOf('--arch');
const archArg = archIndex >= 0 && process.argv[archIndex + 1] ? process.argv[archIndex + 1] : '';
const hostArch = process.arch === 'arm64' ? 'arm64' : process.arch === 'x64' ? 'x64' : process.arch;
const arch = archArg || hostArch;
if (!['arm64', 'x64'].includes(arch)) {
  console.error(`不支持的 --arch：${archArg || process.arch}（支持 x64 / arm64）`);
  process.exit(1);
}
// rust 目标三元组：macOS 用 x86_64（不是 x64）
const triples = {
  darwin: { arm64: 'aarch64-apple-darwin', x64: 'x86_64-apple-darwin' },
  win32: { arm64: 'aarch64-pc-windows-msvc', x64: 'x86_64-pc-windows-msvc' },
  linux: { arm64: 'aarch64-unknown-linux-gnu', x64: 'x86_64-unknown-linux-gnu' },
};
const triple = triples[process.platform]?.[arch];
if (!triple) {
  console.error(`平台 ${process.platform} 不支持架构 ${arch}`);
  process.exit(1);
}
const targetArgs = arch === hostArch ? [] : ['--target', triple];

if (!fs.existsSync(manifest)) {
  console.error(`native/Cargo.toml 不存在：${manifest}`);
  process.exit(1);
}

console.log(`native 模式：cargo build --release${targetArgs.length ? `（${triple}）` : ''} → lib/…`);
const build = spawnSync('cargo', ['build', '--release', '--manifest-path', manifest, ...targetArgs], {
  // stdin 必须忽略：某些机器（安全软件挂钩匿名管道）上给子进程接 stdin 管道
  // 会让 CreateProcess 直接 EBUSY（与 build-web.mjs 同因）。
  stdio: ['ignore', 'inherit', 'inherit'],
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

// 平台产物命名（cargo 惯例）：macOS/Linux 带 lib 前缀，Windows 没有；
// 交叉编译产物在 target/<triple>/release 下。
const targetDir = targetArgs.length
  ? path.join(repoRoot, 'native', 'target', triple, 'release')
  : path.join(repoRoot, 'native', 'target', 'release');
const platform = process.platform;
const artifact =
  platform === 'darwin'
    ? path.join(targetDir, `lib${libName}.dylib`)
    : platform === 'win32'
      ? path.join(targetDir, `${libName}.dll`)
      : path.join(targetDir, `lib${libName}.so`);

if (!fs.existsSync(artifact)) {
  console.error(`构建产物不存在：${artifact}`);
  process.exit(1);
}

const libDir = path.join(repoRoot, 'lib');
fs.mkdirSync(libDir, { recursive: true });

// 两个名字：带架构后缀（pack-plugin.mjs --platform/--arch 的源文件约定名）+
// 宿主默认名（只在本机架构下产出，quickDev / 合包兜底用）。
const archSuffixName =
  platform === 'darwin'
    ? (arch === 'arm64' ? 'macOS-arm64.dylib' : 'macOS-x86_64.dylib')
    : platform === 'win32'
      ? (arch === 'arm64' ? 'Windows-arm64.dll' : 'Windows-x64.dll')
      : (arch === 'arm64' ? 'Linux-arm64.so' : 'Linux-x64.so');
const hostDefaultName =
  platform === 'darwin' ? 'macOS.dylib' : platform === 'win32' ? 'Windows.dll' : 'Linux.so';

fs.copyFileSync(artifact, path.join(libDir, archSuffixName));
const size = (fs.statSync(path.join(libDir, archSuffixName)).size / 1048576).toFixed(1);
console.log(`已拷贝 ${artifact} → lib/${archSuffixName}（${size} MB）`);
if (arch === hostArch) {
  fs.copyFileSync(artifact, path.join(libDir, hostDefaultName));
  console.log(`已拷贝 → lib/${hostDefaultName}（宿主 quickDev / 合包兜底用）`);
} else {
  console.log(`交叉架构 ${arch}：不覆盖 lib/${hostDefaultName}（宿主默认名只放本机架构）。`);
}
console.log('\n完成。单架构打包：pack-plugin.mjs --platform/--arch；合包：三平台默认名库 + pack。');
