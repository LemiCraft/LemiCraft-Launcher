// Texts of the profile page: raw values from the site (milliseconds, ISO dates) turned into what a player reads

const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];

function dateOf(iso) {
  if (!iso) return null;
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? null : date;
}

const pad = (n) => String(n).padStart(2, '0');

// "248 ч 21 мин", "49 мин"
export function formatDuration(ms) {
  if (ms == null) return '—';
  const minutes = Math.floor(ms / 60000);
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours} ч ${minutes % 60} мин` : `${minutes} мин`;
}

// "24.02.2026"
export function formatDate(iso) {
  const date = dateOf(iso);
  return date ? `${pad(date.getDate())}.${pad(date.getMonth() + 1)}.${date.getFullYear()}` : '—';
}

// "сегодня, 08:15", "вчера, 23:50", "3 ноября, 23:50" (with the year when it is not this one)
export function formatLastLogin(iso, now = new Date()) {
  const date = dateOf(iso);
  if (!date) return '—';
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  const startOfDay = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(date)) / 86400000);
  if (days === 0) return `сегодня, ${time}`;
  if (days === 1) return `вчера, ${time}`;
  const year = date.getFullYear() === now.getFullYear() ? '' : ` ${date.getFullYear()}`;
  return `${date.getDate()} ${MONTHS[date.getMonth()]}${year}, ${time}`;
}

// "только что", "49 мин назад", "3 ч назад", "вчера", "5 дн. назад", "3 нед. назад"
export function formatAgo(iso, now = Date.now()) {
  const date = dateOf(iso);
  if (!date) return '';
  const minutes = Math.floor((now - date.getTime()) / 60000);
  if (minutes < 1) return 'только что';
  if (minutes < 60) return `${minutes} мин назад`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} ч назад`;
  const days = Math.floor(hours / 24);
  if (days === 1) return 'вчера';
  if (days < 7) return `${days} дн. назад`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks} нед. назад`;
  return `${Math.floor(days / 30)} мес. назад`;
}

// "1 180"
export function formatCount(value) {
  return value == null ? '—' : Number(value).toLocaleString('ru-RU');
}

export function formatPing(value) {
  return value == null ? '—' : `${Math.round(value)} мс`;
}

// Text on a role chip: dark on a light colour, light on a dark one
export function roleTextColor(colors) {
  const rgb = (colors ?? [])
    .map((c) => /^#?([0-9a-f]{6})$/i.exec(c ?? '')?.[1])
    .filter(Boolean)
    .map((hex) => [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16)));
  if (!rgb.length) return 'var(--text)';
  const mean = [0, 1, 2].map((i) => rgb.reduce((sum, c) => sum + c[i], 0) / rgb.length);
  const luminance = (0.299 * mean[0] + 0.587 * mean[1] + 0.114 * mean[2]) / 255;
  return luminance > 0.6 ? '#101014' : '#ffffff';
}

export function roleBackground(colors) {
  // what the site sends goes into CSS, so only plain hex colours pass
  const list = (colors ?? []).filter((c) => /^#[0-9a-f]{6}$/i.test(c ?? ''));
  if (!list.length) return 'var(--surface-2)';
  return list.length === 1 ? list[0] : `linear-gradient(90deg, ${list[0]}, ${list[1]})`;
}
