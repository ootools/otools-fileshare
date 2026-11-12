export function pathName(path) {
  return path.replace(/\\/g, '/').split('/').filter(Boolean).at(-1) || path;
}

export async function sharePaths(api, paths, { start = true } = {}) {
  for (const path of new Set(paths.filter(path => typeof path === 'string' && path))) {
    await api.addFile({ name: pathName(path), path });
  }
  if (start && paths.length && await api.getServerStatus() === 'stop') await api.startServer();
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
