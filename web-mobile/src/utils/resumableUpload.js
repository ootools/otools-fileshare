export async function uploadWithResume(file, { fetcher = fetch, storage = localStorage, chunkSize = 20 * 1024 * 1024, token = '', onProgress = () => {} } = {}) {
  const key = `fileshare-upload:${file.name}:${file.size}:${file.lastModified}`;
  const headers = { 'Tus-Resumable': '1.0.0', Authorization: token };
  let url = storage.getItem(key);
  let offset = 0;
  const getOffset = async () => {
    const response = await fetcher(url, { method: 'HEAD', headers });
    if (!response.ok) return false;
    if (Number(response.headers.get('Upload-Length')) !== file.size) return false;
    offset = Number(response.headers.get('Upload-Offset'));
    return Number.isSafeInteger(offset) && offset >= 0 && offset <= file.size;
  };
  if (!url || !url.startsWith('/api/uploads/') || !await getOffset()) {
    const name = btoa(Array.from(new TextEncoder().encode(file.name), byte => String.fromCharCode(byte)).join(''));
    const response = await fetcher('/api/uploads', { method: 'POST', headers: { ...headers, 'Upload-Length': String(file.size), 'Upload-Metadata': `filename ${name}` } });
    if (response.status !== 201) throw new Error(`创建上传失败 (${response.status})`);
    url = response.headers.get('Location');
    if (!url?.startsWith('/api/uploads/')) throw new Error('上传地址无效');
    storage.setItem(key, url);
    offset = 0;
  }
  if (!Number.isSafeInteger(chunkSize) || chunkSize <= 0) throw new Error('分片大小无效');
  if (offset === file.size && file.size > 0) {
    storage.removeItem(key);
    onProgress(100);
    return { code: 200, message: '添加成功' };
  }
  let conflicts = 0;
  do {
    const end = Math.min(file.size, offset + chunkSize);
    const response = await fetcher(url, { method: 'PATCH', headers: { ...headers, 'Content-Type': 'application/offset+octet-stream', 'Upload-Offset': String(offset) }, body: file.slice(offset, end) });
    if (response.status === 409) {
      if (++conflicts > 3 || !await getOffset()) throw new Error('续传偏移无效');
      if (offset === file.size) break;
      continue;
    }
    if (response.status !== 204) throw new Error(`上传失败 (${response.status})`);
    const next = Number(response.headers.get('Upload-Offset'));
    if (next !== end) throw new Error('服务器上传偏移不匹配');
    offset = next;
    onProgress(file.size ? Math.round(offset / file.size * 100) : 100);
  } while (offset < file.size);
  storage.removeItem(key);
  return { code: 200, message: '添加成功' };
}
