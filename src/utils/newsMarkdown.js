const DISCORD_TS_RE = /<t:(\d+)(?::([tTdDfFRsS]))?>/g;
// A role mention is "@<emoji><Role Name>" and the name can contain spaces with no delimiter —
// an emoji right after "@" means "role mention", so one extra capitalized word is swept in
const MENTION_RE = /@(?:\p{Extended_Pictographic}\u{FE0F}?[^\s,.!?;:)(]*(?:\s+\p{Lu}[^\s,.!?;:)(]*)?|[^\s,.!?;:)(]+)/gu;
// https only — open_external() (the only way these links get opened) now rejects plain http,
// so linkifying it would render a clickable link that silently fails when clicked
const URL_RE = /https:\/\/[^\s<>"]+[^\s<>".,;!?)\]']/gi;

const RU_MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];

function pad(n) {
  return String(n).padStart(2, '0');
}

function formatDiscordTimestamp(unixSeconds, style) {
  const d = new Date(unixSeconds * 1000);
  const date = `${d.getDate()} ${RU_MONTHS[d.getMonth()]} ${d.getFullYear()} г.`;
  const dateShort = `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  switch (style) {
    case 'd':
      return dateShort;
    case 'D':
      return date;
    case 't':
      return time;
    case 'T':
      return `${time}:${pad(d.getSeconds())}`;
    case 's':
      return `${dateShort}, ${time}`;
    case 'S':
      return `${dateShort}, ${time}:${pad(d.getSeconds())}`;
    case 'R': {
      const diffSec = (Date.now() - d.getTime()) / 1000;
      if (diffSec < 60) return `${Math.max(0, Math.floor(diffSec))} сек. назад`;
      if (diffSec < 3600) return `${Math.floor(diffSec / 60)} мин. назад`;
      if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} ч. назад`;
      return `${Math.floor(diffSec / 86400)} дн. назад`;
    }
    case 'f':
    case 'F':
    default:
      return `${date} в ${time}`;
  }
}

export function replaceDiscordTimestamps(text) {
  return text.replace(DISCORD_TS_RE, (_, unix, style) => formatDiscordTimestamp(Number(unix), style));
}

const BARE_URL_RE = /^https?:\/\/[^\s]+$/i;

export function shortDisplayTitle(title) {
  const trimmed = (title || '').trim();
  if (!BARE_URL_RE.test(trimmed)) return title;
  try {
    return new URL(trimmed).host.replace(/^www\./, '');
  } catch {
    return title;
  }
}

function parseInlineStyles(text) {
  const parts = [];
  let current = '';
  let i = 0;
  const flush = () => {
    if (current) parts.push({ text: current });
    current = '';
  };
  while (i < text.length) {
    if (text[i] === '|' && text[i + 1] === '|') {
      const end = text.indexOf('||', i + 2);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + 2, end), spoiler: true });
        i = end + 2;
        continue;
      }
    }
    if (text[i] === '*' && text[i + 1] === '*') {
      const end = text.indexOf('**', i + 2);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + 2, end), bold: true });
        i = end + 2;
        continue;
      }
    }
    if (text[i] === '*' && (i === 0 || text[i - 1] !== '*') && text[i + 1] !== '*') {
      const end = text.indexOf('*', i + 1);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + 1, end), italic: true });
        i = end + 1;
        continue;
      }
    }
    if (text[i] === '~' && text[i + 1] === '~') {
      const end = text.indexOf('~~', i + 2);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + 2, end), strike: true });
        i = end + 2;
        continue;
      }
    }
    if (text[i] === '`') {
      const end = text.indexOf('`', i + 1);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + 1, end), code: true });
        i = end + 1;
        continue;
      }
    }
    current += text[i];
    i++;
  }
  flush();
  return parts;
}

function tokenizeSegment(text) {
  const tokens = [];
  let lastIndex = 0;
  const matches = [...text.matchAll(URL_RE)].map((m) => ({ ...m, kind: 'url' }));
  for (const m of matches) {
    if (m.index > lastIndex) tokens.push(...tokenizeMentions(text.slice(lastIndex, m.index)));
    let host = m[0];
    try {
      host = new URL(m[0]).host.replace(/^www\./, '');
    } catch {
      /* keep raw */
    }
    tokens.push({ kind: 'url', href: m[0], text: host });
    lastIndex = m.index + m[0].length;
  }
  if (lastIndex < text.length) tokens.push(...tokenizeMentions(text.slice(lastIndex)));
  return tokens;
}

function tokenizeMentions(text) {
  const tokens = [];
  let lastIndex = 0;
  for (const m of text.matchAll(MENTION_RE)) {
    if (m.index > lastIndex) tokens.push({ kind: 'text', text: text.slice(lastIndex, m.index) });
    tokens.push({ kind: 'mention', text: m[0] });
    lastIndex = m.index + m[0].length;
  }
  if (lastIndex < text.length) tokens.push({ kind: 'text', text: text.slice(lastIndex) });
  return tokens;
}

// A styled segment (e.g. **bold**) still gets tokenized for urls/mentions — Discord nests them
// (`# **title**`, `**https://...**`) and a link/mention inside bold text must stay clickable.
export function parseInline(text) {
  return parseInlineStyles(text).flatMap((seg) => {
    const style = seg.bold || seg.italic || seg.strike || seg.code || seg.spoiler
      ? { bold: seg.bold, italic: seg.italic, strike: seg.strike, code: seg.code, spoiler: seg.spoiler }
      : null;
    const tokens = tokenizeSegment(seg.text);
    return style ? tokens.map((t) => ({ ...t, ...style })) : tokens;
  });
}

// Skips any heading that just repeats the title, matching the old app's dedup.
export function parseBlocks(markdown, title) {
  const normalizedTitle = (title || '').trim().toLowerCase();
  const lines = replaceDiscordTimestamps(markdown || '').split('\n');
  const blocks = [];
  let paragraph = [];
  let codeLines = null;

  const flushParagraph = () => {
    if (paragraph.length) {
      blocks.push({ type: 'paragraph', lines: paragraph });
      paragraph = [];
    }
  };

  for (const raw of lines) {
    const line = raw.trim();

    if (line.startsWith('```')) {
      if (codeLines !== null) {
        blocks.push({ type: 'code', text: codeLines.join('\n') });
        codeLines = null;
      } else {
        flushParagraph();
        codeLines = [];
      }
      continue;
    }
    if (codeLines !== null) {
      codeLines.push(raw);
      continue;
    }

    const headingMatch = line.match(/^(#{1,3})\s*(.*)$/);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const text = headingMatch[2].trim();
      if (text.toLowerCase() !== normalizedTitle) {
        flushParagraph();
        blocks.push({ type: 'heading', level, text });
      }
      continue;
    }

    if (line.startsWith('>')) {
      flushParagraph();
      blocks.push({ type: 'quote', text: line.replace(/^>\s?/, '') });
      continue;
    }

    if (!line) {
      flushParagraph();
      continue;
    }

    paragraph.push(raw);
  }
  flushParagraph();
  return blocks;
}
