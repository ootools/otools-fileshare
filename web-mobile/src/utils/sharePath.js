export function encodeSharePath(segments) {
  return '/' + segments.map(segment => encodeURIComponent(segment)).join('/');
}

export function decodeSharePath(path) {
  return path.split('/').filter(Boolean).map(segment => decodeURIComponent(segment));
}
