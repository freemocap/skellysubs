import { describe, expect, it } from 'vitest';
import { detectSystem, featuredInstallers, hintedArchitecture, installers, matchingInstallers, parseRelease, recommend, RELEASES_URL } from './downloads';
import type { Architecture, OperatingSystem, Release } from './downloads';

describe('device detection', () => {
  it.each([
    ['Mozilla/5.0 (Linux; Android 15; Pixel 9)', 'Linux aarch64', 5, 'android', 'unknown'],
    ['Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)', 'iPhone', 5, 'ios', 'unknown'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15)', 'MacIntel', 5, 'ios', 'unknown'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15)', 'MacIntel', 0, 'macos', 'unknown'],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32', 0, 'windows', 'unknown'],
    ['Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64', 0, 'linux', 'x64'],
    ['Mozilla/5.0 (X11; Linux aarch64)', 'Linux aarch64', 0, 'linux', 'arm64'],
    ['Mozilla/5.0 (X11; CrOS x86_64 1)', 'Linux x86_64', 0, 'unknown', 'unknown'],
    ['unknown', '', 0, 'unknown', 'unknown'],
  ])('classifies %s without guessing a desktop CPU', (userAgent, platform, maxTouchPoints, os, arch) => {
    expect(detectSystem({ userAgent, platform, maxTouchPoints })).toEqual({ os, arch });
  });

  it('requires a known 64-bit client hint', () => {
    expect(hintedArchitecture({ architecture: 'arm', bitness: '64' })).toBe('arm64');
    expect(hintedArchitecture({ architecture: 'x86', bitness: '64' })).toBe('x64');
    expect(hintedArchitecture({ architecture: 'x86', bitness: '32' })).toBe('unknown');
    expect(hintedArchitecture({ architecture: '', bitness: '' })).toBe('unknown');
  });
  it('uses explicit 32-bit Android hints without guessing from device models', () => {
    expect(hintedArchitecture({ architecture: 'arm', bitness: '32' }, 'android')).toBe('armv7');
    expect(hintedArchitecture({ architecture: 'x86', bitness: '32' }, 'android')).toBe('x86');
    expect(hintedArchitecture({ architecture: 'arm', bitness: '' }, 'android')).toBe('unknown');
  });
});

// Filenames follow the desktop and Android workflow outputs.
const names = [
  'SkellySpeak_0.9.1_aarch64.dmg', 'SkellySpeak_0.9.1_x64.dmg',
  'SkellySpeak_0.9.1_x64-setup.exe', 'SkellySpeak_0.9.1_x64_en-US.msi',
  'SkellySpeak_0.9.1_amd64.AppImage', 'SkellySpeak_0.9.1_amd64.deb',
  'SkellySpeak_0.9.1_arm64.deb', 'SkellySpeak-0.9.1-1.x86_64.rpm',
  'SkellySpeak_0.9.1_universal.apk', 'SkellySpeak_0.9.1.ipa', 'SkellySpeak_0.9.1.aab',
  'SkellySpeak_0.9.1_arm64-v8a.apk', 'SkellySpeak_0.9.1_armeabi-v7a.apk',
  'SkellySpeak_0.9.1_x86.apk', 'SkellySpeak_0.9.1_x86_64.apk',
  'latest.json', 'SkellySpeak_aarch64.app.tar.gz', 'SkellySpeak_0.9.1_amd64.AppImage.sig',
];
const release: Release = {
  tag_name: 'v0.9.1', html_url: `${RELEASES_URL}/tag/v0.9.1`,
  assets: names.map(name => ({ name, size: 1024, browser_download_url: `${RELEASES_URL}/download/v0.9.1/${name}` })),
};

describe('published installer selection', () => {
  it('exposes ARM64 beside Universal when Android hides its processor without claiming a match', () => {
    const system = detectSystem({ userAgent: 'Mozilla/5.0 (Linux; Android 10; K)', platform: 'Linux armv81', maxTouchPoints: 5 });
    system.arch = hintedArchitecture({ architecture: '', bitness: '' }, system.os);
    const available = installers(release);
    expect(system.arch).toBe('unknown');
    expect(featuredInstallers(available, system).map(item => item.arch)).toEqual(['arm64', 'universal']);
    expect(recommend(available, system)?.arch).toBe('universal');
    expect(featuredInstallers(available.filter(item => item.arch === 'universal'), system).map(item => item.arch)).toEqual(['universal']);
    expect(featuredInstallers([], system)).toEqual([]);
  });
  it.each(['arm64', 'armv7', 'x86', 'x64'] as const)('keeps the matching Android APK and Universal visible: %s', arch => {
    expect(featuredInstallers(installers(release), { os: 'android', arch }).map(item => item.arch)).toEqual([arch, 'universal']);
  });
  it('excludes updater and store artifacts', () => {
    expect(installers(release)).toHaveLength(13);
    expect(installers(release).map(item => item.format)).not.toContain('IPA');
  });

  it.each<[OperatingSystem, Architecture, string | null]>([
    ['windows', 'x64', 'EXE'], ['macos', 'arm64', 'DMG'], ['macos', 'x64', 'DMG'],
    ['linux', 'x64', 'AppImage'], ['linux', 'arm64', 'DEB'], ['android', 'unknown', 'APK'],
    ['ios', 'arm64', null], ['windows', 'arm64', null], ['macos', 'unknown', 'DMG'], ['windows', 'unknown', 'EXE'], ['unknown', 'unknown', null],
  ])('recommends only a matching %s / %s installer', (os, arch, format) => {
    const choice = recommend(installers(release), { os, arch });
    expect(choice?.format ?? null).toBe(format);
    if (choice) expect(arch === 'unknown' || choice.arch === arch || choice.arch === 'universal').toBe(true);
  });

  it('does not invent a link for a missing artifact', () => {
    expect(recommend(installers({ ...release, assets: [] }), { os: 'android', arch: 'unknown' })).toBeNull();
  });
  it.each(['arm64', 'armv7', 'x86', 'x64'] as const)('prefers the exact Android ABI over universal: %s', arch => {
    const available = installers(release);
    expect(recommend(available, { os: 'android', arch })?.arch).toBe(arch);
    expect(recommend(available.filter(item => item.arch !== arch), { os: 'android', arch })?.arch).toBe('universal');
    expect(recommend(available.filter(item => item.arch !== arch && item.arch !== 'universal'), { os: 'android', arch })).toBeNull();
  });
  it('uses only universal for unidentified Android devices, including older releases', () => {
    const available = installers(release);
    const system = { os: 'android', arch: 'unknown' } as const;
    expect(recommend(available, system)?.arch).toBe('universal');
    expect(recommend(available.filter(item => item.arch !== 'universal'), system)).toBeNull();
    expect(recommend(available.filter(item => item.arch === 'universal'), { os: 'android', arch: 'arm64' })?.arch).toBe('universal');
  });

  it('validates release data and rejects drafts, prereleases and foreign links', () => {
    const payload = { ...release, draft: false, prerelease: false };
    expect(parseRelease(payload)).toEqual(release);
    expect(() => parseRelease({ ...payload, draft: true })).toThrow();
    expect(() => parseRelease({ ...payload, prerelease: true })).toThrow();
    expect(() => parseRelease({ ...payload, assets: [{ ...release.assets[0], browser_download_url: 'https://example.com/app.exe' }] })).toThrow();
    expect(() => parseRelease({ message: 'API rate limit exceeded' })).toThrow();
  });
});

it.each(['x64', 'arm64'] as const)('shows only the matching Mac processor: %s', arch => {
  const available = installers(release);
  const matching = matchingInstallers(available, { os: 'macos', arch });
  expect(matching).toHaveLength(1);
  expect(matching[0].arch).toBe(arch);
  expect(recommend(available.filter(item => item.arch !== arch), { os: 'macos', arch })).toBeNull();
});
it('recommends Apple Silicon when the Mac browser withholds its processor', () => {
  const system = { os: 'macos', arch: 'unknown' } as const;
  expect(matchingInstallers(installers(release), system).map(item => item.arch)).toEqual(['arm64', 'x64']);
  expect(recommend(installers(release), system)?.arch).toBe('arm64');
});
it('shows all available packages without a recommendation when the OS is unknown', () => {
  const system = { os: 'unknown', arch: 'unknown' } as const;
  expect(matchingInstallers(installers(release), system)).toEqual(installers(release));
  expect(recommend(installers(release), system)).toBeNull();
});
