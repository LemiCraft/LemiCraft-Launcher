const DISCORD_TS_RE = /<t:(\d+)(?::([tTdDfFRsS]))?>/g;
// A role mention is "@<emoji><Role Name>"; a second capitalized word isn't swept in (it swallowed the next sentence's start)
const MENTION_RE = /@(?:\p{Extended_Pictographic}\u{FE0F}?[^\s,.!?;:)(]*|[^\s,.!?;:)(]+)/gu;
// https only: open_external() rejects plain http, so such a link would be clickable but silently fail
const URL_RE = /https:\/\/[^\s<>"]+[^\s<>".,;!?)\]']/gi;
// Discord custom emoji: <:name:id>, animated <a:name:id>
const CUSTOM_EMOJI_RE = /<(a?):(\w{2,32}):(\d{15,25})>/g;
// a post cut off in the middle of one leaves a dangling "<:name:12"
const CUT_EMOJI_RE = /<a?:\w*(?::\d*)?$/;

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

const WORD_CHAR_RE = /[\p{L}\p{N}]/u;

// Discord's _italic_ only counts at word edges, so snake_case and links like /a_b_c stay as they are
function underscoreItalicEnd(text, start) {
  const before = text[start - 1];
  if (before && (WORD_CHAR_RE.test(before) || '/=?&#'.includes(before))) return -1;
  const first = text[start + 1];
  if (!first || first === ' ' || first === '_') return -1;
  for (let j = start + 2; j < text.length; j++) {
    if (text[j] === '_' && text[j - 1] !== ' ' && !(text[j + 1] && WORD_CHAR_RE.test(text[j + 1]))) return j;
  }
  return -1;
}

const ESCAPABLE_RE = /[\\*_~|`>#[\]()<-]/;

// the closing pair of "**a *b***" is the last two stars of the run
function findBoldEnd(text, from) {
  let end = text.indexOf('**', from);
  if (end < 0) return -1;
  while (text[end + 2] === '*') end++;
  return end;
}

// Discord's *italic* needs a non-space after the opening star and before the closing one, so "5 * 3 * 2" stays plain
function starItalicEnd(text, start) {
  const first = text[start + 1];
  if (!first || first === ' ' || first === '*') return -1;
  for (let j = start + 2; j < text.length; j++) {
    if (text[j] === '*' && text[j - 1] !== ' ' && text[j - 1] !== '*' && text[j + 1] !== '*') return j;
  }
  return -1;
}

// Styles nest (**bold _italic_**, __**both**__): the inner text is parsed again and inherits the outer style
function parseInlineStyles(text, inherited = {}) {
  const parts = [];
  let current = '';
  let i = 0;
  const flush = () => {
    if (current) parts.push({ text: current, ...inherited });
    current = '';
  };
  const nested = (inner, style) => {
    flush();
    parts.push(...parseInlineStyles(inner, { ...inherited, ...style }));
  };

  while (i < text.length) {
    const ch = text[i];
    if (ch === '\\' && i + 1 < text.length && ESCAPABLE_RE.test(text[i + 1])) {
      current += text[i + 1];
      i += 2;
      continue;
    }
    if (ch === '`') {
      const fence = text.startsWith('```', i) ? '```' : '`';
      const end = text.indexOf(fence, i + fence.length);
      if (end !== -1) {
        flush();
        parts.push({ text: text.slice(i + fence.length, end), ...inherited, code: true });
        i = end + fence.length;
        continue;
      }
    }
    if (ch === '|' && text[i + 1] === '|') {
      const end = text.indexOf('||', i + 2);
      if (end > i + 2) {
        nested(text.slice(i + 2, end), { spoiler: true });
        i = end + 2;
        continue;
      }
    }
    if (text.startsWith('***', i)) {
      const end = text.indexOf('***', i + 3);
      if (end > i + 3) {
        nested(text.slice(i + 3, end), { bold: true, italic: true });
        i = end + 3;
        continue;
      }
    }
    if (ch === '*' && text[i + 1] === '*') {
      const end = findBoldEnd(text, i + 2);
      if (end > i + 2) {
        nested(text.slice(i + 2, end), { bold: true });
        i = end + 2;
        continue;
      }
    }
    if (ch === '_' && text[i + 1] === '_') {
      const end = text.indexOf('__', i + 2);
      if (end > i + 2) {
        nested(text.slice(i + 2, end), { underline: true });
        i = end + 2;
        continue;
      }
    }
    if (ch === '~' && text[i + 1] === '~') {
      const end = text.indexOf('~~', i + 2);
      if (end > i + 2) {
        nested(text.slice(i + 2, end), { strike: true });
        i = end + 2;
        continue;
      }
    }
    if (ch === '*') {
      const end = starItalicEnd(text, i);
      if (end !== -1) {
        nested(text.slice(i + 1, end), { italic: true });
        i = end + 1;
        continue;
      }
    }
    if (ch === '_' && text[i + 1] !== '_') {
      const end = underscoreItalicEnd(text, i);
      if (end !== -1) {
        nested(text.slice(i + 1, end), { italic: true });
        i = end + 1;
        continue;
      }
    }
    current += ch;
    i++;
  }
  flush();
  return parts;
}

function tokenizeSegment(text, mentions) {
  const tokens = [];
  let lastIndex = 0;
  const matches = [...text.matchAll(URL_RE)].map((m) => ({ ...m, kind: 'url' }));
  for (const m of matches) {
    if (m.index > lastIndex) tokens.push(...tokenizeMentions(text.slice(lastIndex, m.index), mentions));
    let host = m[0];
    try {
      host = new URL(m[0]).host.replace(/^www\./, '');
    } catch {
      /* keep raw */
    }
    tokens.push({ kind: 'url', href: m[0], text: host });
    lastIndex = m.index + m[0].length;
  }
  if (lastIndex < text.length) tokens.push(...tokenizeMentions(text.slice(lastIndex), mentions));
  return tokens;
}

function escapeRegExp(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Matches the backend's resolved mentions exactly (longest first); the fuzzy regex is only a fallback for stale cached items
function mentionRegexFor(mentions) {
  if (!mentions || mentions.length === 0) return MENTION_RE;
  const sorted = [...mentions].map((m) => m.raw).filter(Boolean).sort((a, b) => b.length - a.length);
  return sorted.length ? new RegExp(sorted.map(escapeRegExp).join('|'), 'g') : MENTION_RE;
}

// Through the site's image proxy, like on the site: Discord's CDN is not reachable for everyone
function emojiUrl(id) {
  const source = `https://cdn.discordapp.com/emojis/${id}.webp?size=48`;
  return `https://lemicraft.ru/api/proxy/image?url=${encodeURIComponent(source)}`;
}

function tokenizeEmoji(text) {
  const tokens = [];
  let lastIndex = 0;
  for (const m of text.matchAll(CUSTOM_EMOJI_RE)) {
    if (m.index > lastIndex) tokens.push({ kind: 'text', text: text.slice(lastIndex, m.index) });
    tokens.push({ kind: 'emoji', name: m[2], src: emojiUrl(m[3]) });
    lastIndex = m.index + m[0].length;
  }
  const rest = text.slice(lastIndex).replace(CUT_EMOJI_RE, '');
  if (rest) tokens.push({ kind: 'text', text: rest });
  return tokens;
}

function tokenizeMentions(text, mentions) {
  const tokens = [];
  let lastIndex = 0;
  for (const m of text.matchAll(mentionRegexFor(mentions))) {
    if (m.index > lastIndex) tokens.push(...tokenizeEmoji(text.slice(lastIndex, m.index)));
    tokens.push({ kind: 'mention', text: m[0] });
    lastIndex = m.index + m[0].length;
  }
  if (lastIndex < text.length) tokens.push(...tokenizeEmoji(text.slice(lastIndex)));
  return tokens;
}

// [label](https://…) as Discord shows it in embeds
const MASKED_LINK_RE = /\[([^\]\n]+)\]\((https:\/\/[^\s)]+)\)/g;
// <https://…> only switches the link preview off in Discord, the brackets are not shown
const ANGLE_URL_RE = /<(https:\/\/[^\s<>]+)>/g;

// A styled segment (e.g. **bold**) is still tokenized for urls/mentions: Discord nests them, and they must stay clickable
function parseStyled(text, mentions) {
  return parseInlineStyles(text).flatMap((seg) => {
    const { text: segText, ...style } = seg;
    const tokens = tokenizeSegment(segText, mentions);
    return Object.keys(style).length ? tokens.map((t) => ({ ...t, ...style })) : tokens;
  });
}

export function parseInline(text, mentions) {
  const source = (text || '').replace(ANGLE_URL_RE, '$1');
  const tokens = [];
  let lastIndex = 0;
  for (const m of source.matchAll(MASKED_LINK_RE)) {
    if (m.index > lastIndex) tokens.push(...parseStyled(source.slice(lastIndex, m.index), mentions));
    tokens.push({ kind: 'url', href: m[2], text: m[1] });
    lastIndex = m.index + m[0].length;
  }
  if (lastIndex < source.length) tokens.push(...parseStyled(source.slice(lastIndex), mentions));
  return tokens;
}

// A card preview has no blockquote styling, so a quote marker just becomes a line break
export function previewText(text) {
  return (text || '')
    .split('\n')
    .map((line) => line.replace(/^\s*(?:>\s?)+/, '').trimEnd())
    .join('\n')
    .replace(/\n{2,}/g, '\n')
    .trim();
}

// Skips any heading that just repeats the title, matching the old app's dedup.
export function parseBlocks(markdown, title) {
  const normalizedTitle = (title || '').trim().toLowerCase();
  const lines = replaceDiscordTimestamps(markdown || '').split('\n');
  const blocks = [];
  let paragraph = [];
  let codeLines = null;
  let quoteRest = false;

  const flushParagraph = () => {
    if (paragraph.length) {
      blocks.push({ type: 'paragraph', lines: paragraph });
      paragraph = [];
    }
  };

  for (const raw of lines) {
    const line = raw.trim();

    // ```one line```
    if (codeLines === null && line.length > 6 && line.startsWith('```') && line.endsWith('```')) {
      flushParagraph();
      blocks.push({ type: 'code', text: line.slice(3, -3).trim() });
      continue;
    }

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

    // ">>> " quotes everything that follows
    if (quoteRest || line.startsWith('>>>')) {
      flushParagraph();
      quoteRest = true;
      const rest = line.replace(/^>>>\s?/, '');
      if (rest) blocks.push({ type: 'quote', text: rest });
      continue;
    }

    if (line.startsWith('-# ')) {
      flushParagraph();
      blocks.push({ type: 'subtext', text: line.slice(3) });
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

    paragraph.push(raw.replace(/^(\s*)[-*]\s+/, '$1• '));
  }
  flushParagraph();
  // a fence that is never closed still shows what was written
  if (codeLines !== null) blocks.push({ type: 'code', text: codeLines.join('\n') });
  return blocks;
}
