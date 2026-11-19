import { describe, expect, it } from 'vitest';
import { formatAgo, formatCount, formatDuration, formatLastLogin, formatPing, roleBackground, roleTextColor } from './profileFormat.js';

describe('the time in the game', () => {
  it('is hours and minutes, or minutes alone', () => {
    expect(formatDuration(894_070_530)).toBe('248 ч 21 мин');
    expect(formatDuration(49 * 60_000)).toBe('49 мин');
    expect(formatDuration(0)).toBe('0 мин');
  });

  it('is a dash when there is nothing', () => {
    expect(formatDuration(null)).toBe('—');
    expect(formatDuration(undefined)).toBe('—');
  });
});

describe('numbers', () => {
  it('count and ping have a dash for nothing and keep the thousands apart', () => {
    expect(formatCount(null)).toBe('—');
    expect(formatCount(1180).replace(/\s/g, ' ')).toBe('1 180');
    expect(formatPing(95.4)).toBe('95 мс');
    expect(formatPing(null)).toBe('—');
  });
});

describe('how long ago', () => {
  const now = Date.parse('2026-11-05T12:00:00Z');
  const ago = (ms) => new Date(now - ms).toISOString();

  it('is told in the unit that fits', () => {
    expect(formatAgo(ago(20_000), now)).toBe('только что');
    expect(formatAgo(ago(49 * 60_000), now)).toBe('49 мин назад');
    expect(formatAgo(ago(3 * 3_600_000), now)).toBe('3 ч назад');
    expect(formatAgo(ago(30 * 3_600_000), now)).toBe('вчера');
    expect(formatAgo(ago(5 * 86_400_000), now)).toBe('5 дн. назад');
    expect(formatAgo(ago(15 * 86_400_000), now)).toBe('2 нед. назад');
  });

  it('is nothing for a date that cannot be read', () => {
    expect(formatAgo(null, now)).toBe('');
    expect(formatAgo('not a date', now)).toBe('');
  });
});

describe('the last login', () => {
  const now = new Date(2026, 10, 5, 14, 0);

  it('says today and yesterday with the time', () => {
    expect(formatLastLogin(new Date(2026, 10, 5, 8, 15).toISOString(), now)).toBe('сегодня, 08:15');
    expect(formatLastLogin(new Date(2026, 10, 4, 23, 50).toISOString(), now)).toBe('вчера, 23:50');
  });

  it('names the month, and the year when it is not this one', () => {
    expect(formatLastLogin(new Date(2026, 9, 3, 9, 5).toISOString(), now)).toBe('3 октября, 09:05');
    expect(formatLastLogin(new Date(2025, 11, 31, 22, 0).toISOString(), now)).toBe('31 декабря 2025, 22:00');
  });
});

describe('the colours of a role', () => {
  it('put dark text on a light colour and light text on a dark one', () => {
    expect(roleTextColor(['#F1C40F', '#C0F10F'])).toBe('#101014');
    expect(roleTextColor(['#2136AC', '#1F2753'])).toBe('#ffffff');
  });

  it('make a gradient of two colours and one colour of one', () => {
    expect(roleBackground(['#ff5f5f', '#ff9a5f'])).toBe('linear-gradient(90deg, #ff5f5f, #ff9a5f)');
    expect(roleBackground(['#7c7cff'])).toBe('#7c7cff');
  });

  it('let nothing but a plain hex colour into the CSS', () => {
    expect(roleBackground(['url(https://evil.example/x)', 'red; background: url(x)'])).toBe('var(--surface-2)');
    expect(roleBackground(['#ff5f5f', 'expression(alert(1))'])).toBe('#ff5f5f');
    expect(roleBackground(null)).toBe('var(--surface-2)');
  });
});
