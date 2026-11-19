import { describe, expect, it } from 'vitest';
import { JVM_PROFILES, profileOf, profileRam, recommendedProfile } from './jvmProfiles.js';

const [low, normal, heavy] = JVM_PROFILES;

describe('the profile that suits a machine', () => {
  it('goes by the memory of the machine', () => {
    expect(recommendedProfile(4).id).toBe('low');
    expect(recommendedProfile(6).id).toBe('low');
    expect(recommendedProfile(8).id).toBe('normal');
    expect(recommendedProfile(12).id).toBe('normal');
    expect(recommendedProfile(16).id).toBe('heavy');
  });
});

describe('the memory of a profile', () => {
  it('follows the machine inside its limits', () => {
    expect(profileRam(low, 4)).toBe(2);
    expect(profileRam(low, 32)).toBe(3);
    expect(profileRam(normal, 16)).toBe(4);
    expect(profileRam(heavy, 16)).toBe(8);
    expect(profileRam(heavy, 64)).toBe(10);
  });

  it('is never below the limit even on a machine that reports nothing', () => {
    expect(profileRam(low, 0)).toBe(2);
    expect(profileRam(heavy, 0)).toBe(6);
  });
});

describe('which profile the settings are', () => {
  it('knows a profile it has set', () => {
    for (const profile of JVM_PROFILES) {
      expect(profileOf(profileRam(profile, 16), profile.args, 16)?.id).toBe(profile.id);
    }
  });

  it('does not mind the spaces in the flags', () => {
    expect(profileOf(4, `  ${normal.args.replace(' ', '   ')} `, 16)?.id).toBe('normal');
  });

  it('is the own set as soon as the memory or the flags are changed by hand', () => {
    expect(profileOf(5, normal.args, 16)).toBeNull();
    expect(profileOf(4, `${normal.args} -Dfoo=bar`, 16)).toBeNull();
    expect(profileOf(4, '', 16)).toBeNull();
  });
});

describe('the flags of the profiles', () => {
  it('differ, so that two profiles are never taken for each other', () => {
    expect(new Set(JVM_PROFILES.map((profile) => profile.args)).size).toBe(JVM_PROFILES.length);
  });

  it('keep the usual set as it was before the profiles', () => {
    expect(normal.args).toBe('-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions');
  });
});
