export const RELEASES_URL = 'https://github.com/freemocap/skellyspeak/releases';
export const RELEASE_API = 'https://api.github.com/repos/freemocap/skellyspeak/releases/latest';

export type OperatingSystem = 'windows' | 'macos' | 'linux' | 'android' | 'ios' | 'unknown';
export type Architecture = 'x64' | 'arm64' | 'armv7' | 'x86' | 'unknown';
export interface System { os: OperatingSystem; arch: Architecture }
export interface BrowserIdentity { userAgent: string; platform: string; maxTouchPoints: number }
export interface ClientHints { architecture: string; bitness: string }

export const OS_LABELS: Record<OperatingSystem, string> = {
  windows: 'Windows', macos: 'macOS', linux: 'Linux', android: 'Android', ios: 'iPhone / iPad', unknown: 'Choose your system',
};

// Mobile signatures take precedence over their desktop substrings. Mac and
// Windows user agents can describe an emulated CPU, so they do not select one.
export function detectSystem(browser: BrowserIdentity): System {
  const ua = browser.userAgent.toLowerCase();
  const platform = browser.platform.toLowerCase();
  if (/iphone|ipad|ipod/.test(ua) || (platform === 'macintel' && browser.maxTouchPoints > 1)) return { os: 'ios', arch: 'unknown' };
  if (ua.includes('android')) return { os: 'android', arch: 'unknown' };
  if (ua.includes('cros')) return { os: 'unknown', arch: 'unknown' };
  if (ua.includes('win')) return { os: 'windows', arch: 'unknown' };
  if (ua.includes('mac')) return { os: 'macos', arch: 'unknown' };
  if (ua.includes('linux')) {
    const arch = /aarch64|arm64/.test(ua) ? 'arm64' : /x86_64|amd64/.test(ua) ? 'x64' : 'unknown';
    return { os: 'linux', arch };
  }
  return { os: 'unknown', arch: 'unknown' };
}

export function hintedArchitecture(hints: ClientHints, os: OperatingSystem = 'unknown'): Architecture {
  if (os === 'android' && hints.bitness === '32') {
    if (hints.architecture === 'arm') return 'armv7';
    if (hints.architecture === 'x86') return 'x86';
  }
  if (hints.bitness !== '64') return 'unknown';
  if (hints.architecture === 'arm') return 'arm64';
  if (hints.architecture === 'x86') return 'x64';
  return 'unknown';
}

export interface ReleaseAsset { name: string; browser_download_url: string; size: number }
export interface Release { tag_name: string; html_url: string; assets: ReleaseAsset[] }
export interface Installer {
  asset: ReleaseAsset;
  os: 'windows' | 'macos' | 'linux' | 'android';
  arch: Exclude<Architecture, 'unknown'> | 'universal';
  format: 'EXE' | 'MSI' | 'DMG' | 'AppImage' | 'DEB' | 'RPM' | 'APK';
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('GitHub returned invalid release data.');
  return value as Record<string, unknown>;
}

function releaseLink(value: unknown, prefix: string): string {
  if (typeof value !== 'string' || !value.startsWith(prefix)) throw new Error('GitHub returned an invalid release link.');
  return value;
}

export function parseRelease(value: unknown): Release {
  const release = object(value);
  if (typeof release.tag_name !== 'string' || !release.tag_name || release.draft !== false || release.prerelease !== false || !Array.isArray(release.assets)) {
    throw new Error('GitHub did not return a published stable release.');
  }
  return {
    tag_name: release.tag_name,
    html_url: releaseLink(release.html_url, `${RELEASES_URL}/tag/`),
    assets: release.assets.map((value: unknown) => {
      const asset = object(value);
      if (typeof asset.name !== 'string' || typeof asset.size !== 'number' || !Number.isSafeInteger(asset.size) || asset.size < 0) throw new Error('GitHub returned an invalid release asset.');
      return { name: asset.name, size: asset.size, browser_download_url: releaseLink(asset.browser_download_url, `${RELEASES_URL}/download/`) };
    }),
  };
}

// Match installable outputs from release.yml. Updater archives, signatures,
// AABs and IPAs are not direct-install choices on this page.
export function installers(release: Release): Installer[] {
  return release.assets.flatMap((asset): Installer[] => {
    const name = asset.name.toLowerCase();
    if (!name.startsWith('skellyspeak')) return [];
    if (name.endsWith('_universal.apk')) return [{ asset, os: 'android', arch: 'universal', format: 'APK' }];
    const androidArchitectures = { 'arm64-v8a': 'arm64', 'armeabi-v7a': 'armv7', x86: 'x86', x86_64: 'x64' } as const;
    for (const [abi, arch] of Object.entries(androidArchitectures)) {
      if (name.endsWith(`_${abi}.apk`)) return [{ asset, os: 'android', arch, format: 'APK' }];
    }
    const arch = /(?:_|\.)(?:aarch64|arm64)(?:_|\.|-)/.test(name) ? 'arm64'
      : /(?:_|\.)(?:x64|amd64|x86_64)(?:_|\.|-)/.test(name) ? 'x64' : null;
    if (!arch) return [];
    if (name.endsWith('.dmg')) return [{ asset, os: 'macos', arch, format: 'DMG' }];
    if (name.endsWith('-setup.exe')) return [{ asset, os: 'windows', arch, format: 'EXE' }];
    if (name.endsWith('.msi')) return [{ asset, os: 'windows', arch, format: 'MSI' }];
    if (name.endsWith('.appimage')) return [{ asset, os: 'linux', arch, format: 'AppImage' }];
    if (name.endsWith('.deb')) return [{ asset, os: 'linux', arch, format: 'DEB' }];
    if (name.endsWith('.rpm')) return [{ asset, os: 'linux', arch, format: 'RPM' }];
    return [];
  });
}

export function matchingInstallers(available: Installer[], system: System): Installer[] {
  return available.filter(item => (system.os === 'unknown' || item.os === system.os) && (system.arch === 'unknown' || item.arch === 'universal' || item.arch === system.arch));
}

export function recommend(available: Installer[], system: System): Installer | null {
  if (system.os === 'android') {
    const android = available.filter(item => item.os === 'android');
    return android.find(item => item.arch === system.arch)
      ?? android.find(item => item.arch === 'universal') ?? null;
  }
  const formats: Installer['format'][] = ['EXE', 'DMG', 'AppImage', 'DEB', 'RPM', 'MSI', 'APK'];
  const preferred = system.arch === 'unknown' ? { ...system, arch: system.os === 'macos' ? 'arm64' as const : 'x64' as const } : system;
  return matchingInstallers(available, preferred)
    .filter(item => system.os !== 'unknown' && (preferred.arch !== 'unknown' || item.arch === 'universal'))
    .sort((a, b) => formats.indexOf(a.format) - formats.indexOf(b.format))[0] ?? null;
}

export function architectureLabel(arch: Installer['arch'], os: OperatingSystem): string {
  if (arch === 'universal') return 'Universal';
  if (os === 'android') return { arm64: 'ARM64 (arm64-v8a)', armv7: 'ARM 32-bit (armeabi-v7a)', x86: 'Intel / AMD 32-bit (x86)', x64: 'Intel / AMD 64-bit (x86_64)' }[arch];
  if (os === 'macos') return arch === 'arm64' ? 'Apple Silicon' : 'Intel';
  return arch === 'arm64' ? 'ARM64' : 'Intel / AMD 64-bit';
}

// Keep the compact Android option and the compatible fallback visible together.
// Showing ARM64 when detection is unavailable is not evidence of compatibility.
export function featuredInstallers(available: Installer[], system: System): Installer[] {
  const recommended = recommend(available, system);
  if (system.os !== 'android') return recommended ? [recommended] : [];
  const arch = system.arch === 'unknown' ? 'arm64' : system.arch;
  return [
    available.find(item => item.os === 'android' && item.arch === arch),
    available.find(item => item.os === 'android' && item.arch === 'universal'),
  ].filter((item): item is Installer => item !== undefined);
}
