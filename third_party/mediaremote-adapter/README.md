# mediaremote-adapter (vendored)

- Upstream: https://github.com/ungive/mediaremote-adapter
- Pinned commit: `0e82bf1c44ec39554fbfc402ec88a48c6ae2a0fd` (2026-10-17, "Sort projects alphabetically")
- License: BSD 3-Clause, see `LICENSE` (also listed in `/THIRD_PARTY_NOTICES.md`).

Ferrocast's notch player runs `/usr/bin/perl bin/mediaremote-adapter.pl MediaRemoteAdapter.framework stream`
to read macOS Now Playing. The framework is bundled but never linked into the Ferrocast binary.

## What is vendored

`bin/`, `include/` and `src/{adapter,private,utility}` copied unmodified from the commit above.

## Local changes

- Not vendored: the CMake build, the test client (`src/test/`), `src/adapter/test.m` (the `test`
  command needs the test client, which Ferrocast does not ship), docs, scripts and editor settings.
- `Info.plist` is new (CMake generates one upstream): the framework bundle's metadata.
- No upstream source file is edited.

## Build

There is no CMake step. `scripts/bundle.sh` compiles the sources with clang (`-fobjc-arc
-fvisibility=default`, linking Foundation, AppKit and UniformTypeIdentifiers when present) into
`MediaRemoteAdapter.framework` (Versions/A layout) for the arch of the Ferrocast binary, and signs it.

## Updating

Replace the files from a newer upstream commit, update the hash above, and re-run the bundle and the
`get` check in `docs/IMPLEMENTATION.md`.
