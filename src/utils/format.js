export function formatFileSize(bytes) {
  if (!bytes) return 'N/A';
  const units = ['B', 'KB', 'MB', 'GB'];
  let len = bytes;
  let order = 0;
  while (len >= 1024 && order < units.length - 1) {
    order++;
    len /= 1024;
  }
  const rounded = Math.round(len * 10) / 10;
  return `${rounded % 1 === 0 ? rounded.toFixed(0) : rounded.toFixed(1)} ${units[order]}`;
}
