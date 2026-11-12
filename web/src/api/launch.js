export function pathName(path) {
  return path.replace(/\\/g, '/').split('/').filter(Boolean).at(-1) || path;
}

export async function sharePaths(api, paths, { start = true, username } = {}) {
  const cleaned = [...new Set(paths.filter(path => typeof path === 'string' && path))];
  for (const path of cleaned) {
    await api.addFile(username ? { name: pathName(path), path, username } : { name: pathName(path), path });
  }
  if (start && cleaned.length && await api.getServerStatus() === 'stop') await api.startServer();
}

export async function handleLaunch(api, launch) {
  if (launch?.type === 'files' && Array.isArray(launch.payload)) {
    await sharePaths(api, launch.payload.map(file => typeof file === 'string' ? file : file.path));
  } else if (launch?.type === 'over' && typeof launch.payload === 'string' && launch.payload) {
    await api.addText(launch.payload);
    if (await api.getServerStatus() === 'stop') await api.startServer();
  } else if ((await api.getSetting()).autoStart && await api.getServerStatus() === 'stop') {
    await api.startServer();
  }
}
