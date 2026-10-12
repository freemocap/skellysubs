# Android APK size and architecture downloads

Status: source changes implemented on 2026-10-12; new signed APK builds and
device installation remain unverified. No release or website was published.

## Measured published baseline

Inspected the [v4.0.0 universal APK](https://github.com/freemocap/skellyspeak/releases/tag/v4.0.0),
downloaded with `gh release download v4.0.0 --pattern '*universal.apk'`.
The archive is **309,371,187 bytes (295.04 MiB)**. Its four native libraries are
stored without ZIP compression and account for **306,594,900 bytes (99.1%)**.

| ABI | Native library bytes | Removable static symbol/name tables | Estimated single-ABI APK after stripping |
| --- | ---: | ---: | ---: |
| arm64-v8a | 77,645,440 | 13,803,599 | 66.6 MB |
| armeabi-v7a | 66,425,612 | 11,569,285 | 57.6 MB |
| x86 | 82,551,840 | 10,418,048 | 74.9 MB |
| x86_64 | 79,972,008 | 11,087,934 | 71.7 MB |

Estimates subtract `.symtab`/`.strtab` from the current native library and add
the current 2,776,287-byte non-native/ZIP/signing remainder. They are not measured
new artifacts; packaging, alignment and subsequent source changes affect size.
The universal APK could lose approximately 46.9 MB from stripping alone.
MB here is decimal; the inspection tool also reports binary MiB.

The remainder is mostly Android resources (1,034,348 bytes for `resources.arsc`)
and compressed Java/Kotlin bytecode (972,016 bytes for `classes.dex`). There is
no separate large model, audio corpus, server tree or development dependency
directory in the APK. This archive inventory cannot assign every byte embedded
inside the native binary to its source dependency.

ARM64 ELF section inspection found 36,756,968 bytes of read-only data and
21,137,064 bytes of machine code, plus unwind/relocation information. The local
frontend build includes substantial multilingual fonts (Chinese 7.78 MB,
Japanese 4.35 MB, Korean 3.91 MB); these support the product's language coverage
and are not removed. A precise attribution of embedded data and experiments
with LTO/size optimization remain a separate investigation. No language-specific
asset omissions, panic-policy changes or native-library compression changes
are part of this work.

## Implemented packaging

The release job builds the universal APK and universal AAB, then invokes
Tauri's `--apk --split-per-abi` mode using the same build cache, version and
keystore. Outputs are five standalone, directly installable APKs:

- `SkellySpeak_<version>_universal.apk`
- `SkellySpeak_<version>_arm64-v8a.apk`
- `SkellySpeak_<version>_armeabi-v7a.apk`
- `SkellySpeak_<version>_x86.apk`
- `SkellySpeak_<version>_x86_64.apk`

The existing `SkellySpeak_<version>.aab` remains. These APKs are independent
installers, not base/config split APK sets requiring a separate installer.
Tauri Gradle flavors use `arm64` and `arm`; published filenames use Android's
ABI names to make compatibility explicit.

`CARGO_PROFILE_RELEASE_STRIP=symbols` is scoped to these Android release builds.
It removes static symbol/debug tables, reducing offline native symbolization
information in the distributed binaries; it does not change structured runtime
diagnostics or provider receipts. Debug builds retain their symbols.

Every APK must contain exactly its declared ABI set and the app library for
each ABI, with no static symbol/debug sections. The workflow verifies every
APK's signing certificate, application ID, version name and version code before
upload. The APK inventory is appended to the Actions job summary. Run it locally:

```sh
python tools/android_apk.py path/to/app.apk universal
python tools/android_apk.py path/to/arm64.apk arm64-v8a --require-stripped
python -m unittest discover -s tools -p 'test_android_apk.py'
```

## Implemented download selection

The documentation page recognizes all five filenames and uses Android browser
client hints for architecture/bitness when available. It does not infer an ABI
from a device model or assume all Android devices are ARM64. A known architecture
prefers its exact APK, falling back to universal when absent (including old
releases). The page keeps the matching APK and Universal visible together.
For an unknown processor, ARM64 is shown as an explicit option for ARM64 devices
beside Universal ("if unsure"); neither card claims a detected match or carries
the recommendation badge. Compatibility selection still falls back only to
Universal. Other architectures remain in the selector and expandable alternatives.
A late hints response cannot override
the user's selection. The page explains `adb shell getprop ro.product.cpu.abilist`
as an optional way to inspect supported ABIs.

The October 12 follow-up addresses the smaller APK being hidden when detection
returns no architecture. This is not necessarily a device recognition defect:
[Chromium Android tests](https://chromium.googlesource.com/chromium/src/+/b37a4c8219404240614ef11bb1ef25aeaed488ac/android_webview/javatests/src/org/chromium/android_webview/test/ClientHintsTest.java)
document empty architecture and bitness hints, and
[GrapheneOS](https://grapheneos.org/features) limits high-entropy client hints in
Vanadium. The user's phone/browser values have not been captured, so its specific
cause remains unverified. No device-model guessing or privacy workarounds were added.

## Verification

- Published v4.0.0 archive inspected with the new tool; ELF32 and ELF64 symbol
  totals measured from the packaged libraries.
- APK inspection tests: passed (ABI mismatches, missing app library, stripped
  versus unstripped libraries, ELF32 and ELF64).
- Documentation download tests: 39 passed, including exact-ABI preference,
  unknown processors, visible ARM64/Universal choices, missing architectures and
  older universal-only releases.
- Documentation demo and dependency-security tests, site type-check, docs links,
  site production build, root fast gate and root UI production build: passed.
- Both workflow YAML documents parse, and every Android release shell step
  passes Bash syntax checking. This is not a hosted workflow execution.
- Signed Android build, actual post-change sizes and universal-to-per-ABI device
  upgrade still require the Android build environment and signing credentials.

## Primary references

- [Tauri Android distribution](https://v2.tauri.app/distribute/google-play/):
  supported architectures and per-ABI packaging.
- [Pinned Tauri 2.11.4 Gradle plugin template](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/templates/mobile/android/buildSrc/src/main/kotlin/RustPlugin.kt):
  universal and architecture product flavors with ABI filters.
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#strip):
  stripping defaults and symbol-stripping configuration.
- [Chrome user-agent client hints](https://developer.chrome.com/docs/privacy-security/user-agent-client-hints):
  requesting architecture and bitness from browsers that expose them.
