# Instructions for AI agents

LT Takeover: Open-Source Direct Control, a native mod for Teamfight
Manager 2 (`probe/`, Rust). It lets the player take over one athlete and
play the match. These rules are for any AI agent working in this
repository.

If `AGENTS.local.md` exists next to this file, read it too: it holds that
checkout's own working agreement and takes precedence.

## What kind of claim it is

Say which of these every finding is, in documents and in reports:

- **Seen in game**: someone played it, with the build and game version.
- **Read from the code**: static analysis of the game's executable.
- **Guess**: plausible, untested.
- **Rejected**: tried and wrong.

Never promote one into another: tests, a build or a tool passing does not
show that something works in game. When a finding is corrected, keep the
old one and mark it superseded with the date; do not quietly delete it.

## Direction

- The mod is about playing as one athlete, like a sports game's career
  mode. Judge feature ideas by that.
- Honest mechanics: an athlete's stats may change how the AI behaves or
  something the player can see, never the hidden outcome of the player's
  own aim. Never add input delay.
- Every native hook has to be found again on each game patch. Prefer the
  game's official mod interface (`sdk/`) where it does the job, and say
  what a new hook buys before proposing one.
- The mod is also meant as a clean base for other modders: keep code and
  documents readable by someone new.

## Where things live

| Thing | Place |
| --- | --- |
| Everything that differs between game versions (addresses, byte patterns, fingerprint, effect and item tables) | `probe/src/native_adapter/v<version>/`, verified against `tools/native_profiles/<version>.json` |
| Hook code, shared by all versions | `probe/src/native_adapter/windows/` |
| What is left to do, done and declined | `docs/backlog.md` |
| How the code is organised; how to change common things | `docs/modding.md` |
| Supporting a new game version, step by step | `docs/version-migration.md` |
| An investigation's findings | `docs/investigation-<topic>.md` |
| Notes on each build | `CHANGELOG.md` (newest first) |
| Game copies, disassembly, tool output | `research/` (not in git) |
| Packaged builds and build records | `dist/` (not in git) |

## Rules for native code

- No game address or game version in the shared hook code. Addresses go in
  the version folders only; `tools/verify_native_profile.py` refuses a
  build that breaks this.
- Never reuse an address or a field offset from one game version in
  another. Carry it with the tools (`tools/draft_version.py`) and confirm
  it against that version's executable.
- An executable the mod does not recognise gets no hooks. Keep every check
  that makes that true: fingerprint, expected bytes, layout guards.
- A hook replaces one instruction, runs inside `catch_unwind`, keeps no
  pointer the game passed in, and turns away anything that is not the
  controlled athlete before taking a lock. See "Threads: the rules" in
  `docs/modding.md`.

## Git

- Tracked: code, documentation, licences and notices. Not tracked:
  screenshots, test sheets, design work, research, packaged builds and the
  game's SDK.
- An investigation document under `docs/` is documentation: commit it.
- Releases are tagged `v<version>`.
- Before anything goes public, check for personal paths, account names and
  raw logs.

## Building and releasing

Follow "Build and release" in `docs/version-migration.md`. In short: bump
the version in `probe/Cargo.toml` and `probe/mod.mod_info`, add a
`CHANGELOG.md` entry, then format, lint with warnings denied, test, build,
stage, verify, package and install.

- The verifier must pass for every supported game version.
- The game must be closed while installing.
- The Workshop uploader can reset the English description and rewrite
  `mod.mod_info`; check both after an upload.
