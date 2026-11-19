import { describe, expect, it } from 'vitest';
import { mergeFirstPage } from './news.js';

const card = (id, day) => ({ id, rawDate: `2026-10-${String(day).padStart(2, '0')}T10:00:00Z` });
const ids = (list) => list.map((item) => item.id).join(',');

describe('a fresh first page of the news', () => {
  const loaded = [card('a', 30), card('b', 29), card('c', 28), card('d', 20), card('e', 15), card('f', 10)];

  it('keeps the older cards that scrolling has loaded when the site gives fewer than asked for', () => {
    const merged = mergeFirstPage(loaded, [card('n', 31), card('a', 30), card('b', 29)]);
    expect(ids(merged.items)).toBe('n,a,b,c,d,e,f');
    expect(merged.keepsCursor).toBe(true);
  });

  it('is the whole list when nothing older was loaded', () => {
    const merged = mergeFirstPage([card('a', 30)], [card('a', 30), card('b', 29)]);
    expect(ids(merged.items)).toBe('a,b');
    expect(merged.keepsCursor).toBe(false);
  });

  it('drops a card that left the first page and keeps the tail', () => {
    const merged = mergeFirstPage(loaded, [card('a', 30), card('c', 28)]);
    expect(ids(merged.items)).toBe('a,c,d,e,f');
  });

  it('takes the page as it is when its dates cannot be read', () => {
    const merged = mergeFirstPage(loaded, [{ id: 'x', rawDate: 'nonsense' }]);
    expect(ids(merged.items)).toBe('x');
    expect(merged.keepsCursor).toBe(false);
  });
});
