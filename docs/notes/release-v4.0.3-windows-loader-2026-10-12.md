# v4.0.3 release: Windows test executable loader failure

## Observed failure and proven cause

The [v4.0.3 release run](https://github.com/freemocap/skellyspeak/actions/runs/38266800161)
failed only in `checks / native / compile`, during test inventory after successful
compilation. All other completed validation jobs passed. Native domain execution,
release drafting, desktop/Android packaging and publication were skipped.
The [ordinary CI run on the same commit](https://github.com/freemocap/skellyspeak/actions/runs/38266799696)
had the identical inventory failure and no other failed jobs.

`tests.exe --list --format terse` exited with decimal `3221225785`, Windows
`0xc0000139` (`STATUS_ENTRYPOINT_NOT_FOUND`), before the test harness started.
This is the same loader failure encountered locally during the v4.0.1 repair;
it was not specific to the local machine and was not another speech timeout.

The startup tests added after v4.0.1 cause the library test executable to import
`TaskDialogIndirect`. That entry point requires Common Controls v6. Tauri's
generated resource manifest is linked using `cargo:rustc-link-arg-bins`, which
does not cover the library test executable. The failing executable had no resource
section and therefore no embedded manifest requesting v6. The older system
Common Controls DLL does not export `TaskDialogIndirect`.

Controlled reproduction: the unchanged executable failed to list tests; adding
only Tauri's default Common Controls v6 manifest to a copy with the Windows SDK
manifest tool made that copy list all 1,000 tests successfully. No source, DLL,
environment or test selection changes were needed for that experiment.

## Implemented repair

On Windows MSVC, `native/build.rs` now asks the linker to embed the same Common
Controls dependency for every linked target. Tauri still creates icon/version
resources, with its separate manifest disabled to avoid duplicate resources.
UAC manifest generation is disabled, retaining the previous manifest's absence
of a requested execution level. Other targets retain Tauri's default behavior.

The existing startup tests and CI inventory exercise the actual failure boundary:
the executable must load before any test can pass. No tests are disabled and no
retry or CI bypass is added. Ordinary app linking must also pass because it uses
Tauri's resources alongside the new linker-owned manifest.

Once the loader was repaired, the full application domain exposed two previously
unreachable startup-test failures: `get_startup_state not allowed. Plugin not
found`. Their request URL was hardcoded to `tauri://localhost`, which is not
Tauri's local origin on Windows. The test now obtains the created window's URL
with `window.url()`, preserving the actual IPC authorization checks and adapting
to the configured development/release origin. All three startup regressions then
passed. Application permissions and startup behavior are unchanged.

Relevant contracts: [Cargo build-script link arguments](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-link-arg)
and the locally resolved `tauri-build` 2.6.3 / `tauri-winres` 0.3.6 sources.

## Verification

- Exact CI preparation (`node tools/ci/native-tests.ts prepare`): passed;
  inventories 1,000 tests across 14 domains.
- Startup regressions: all three passed after the request-origin correction.
- Ordinary Windows app build (`cargo build --manifest-path native/Cargo.toml
  --bin skellyspeak`): passed. Extracted manifests from both the built app and CI
  test executable contain the expected Common Controls v6 dependency and no UAC
  execution-level declaration.
- All 14 native domains covered: 995 passed, five existing ignored tests, zero
  failures. AI and the 12 remaining domains ran through the CI domain runner;
  the complete application domain was rerun with Cargo after fixing its fixture
  origin (88 passed). Final CI artifact preparation was repeated successfully
  after that correction. No domain was omitted.
- The speech domain includes the original v4.0.1 failing test and both mock
  timing regressions: 85 passed, one ignored. This resolves the earlier local
  verification limitation recorded in the v4.0.1 investigation.
- Final `npm run check:fast`, strict Clippy (`--lib --tests -- -D warnings`),
  documentation links and diff whitespace checks: passed.

This is local verification, not a packaged release or a green hosted run. No
commit, push, version change, tag or release has been performed.
