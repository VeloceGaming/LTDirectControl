"""Copy the game's mod SDK into sdk/ (not tracked) and add the mod's two
local SDK additions.

The SDK belongs to the game's developer and ships with the game
(<game>/mod-sdk-stable); this project does not redistribute it. Run once
after installing or updating the game, before building:

    python tools/prepare_sdk.py

Local additions (the mod's own code, inserted after fixed anchors; the run
fails if an anchor is missing or ambiguous, e.g. after an SDK update):
- `with_native_context` on StableSim and the AI context: the native
  adapter's access to the host pointers (unsafe, not a stable SDK API).
- The item-build hook export: the SDK builds the hook list but never sets
  its pointer/length in the export, so the game would not see the hooks.
"""
import shutil
import sys
from pathlib import Path

from paths import GAME_DIR

ROOT = Path(__file__).resolve().parents[1]
SOURCE = GAME_DIR / 'mod-sdk-stable'
TARGET = ROOT / 'sdk'

NATIVE_CONTEXT = '''
    /// Local, build-specific adapter access; this is NOT a stable SDK API.
    ///
    /// # Safety
    /// The caller must verify the host executable and native layouts before
    /// dereferencing either pointer. Neither pointer may escape this callback.
    pub unsafe fn with_native_context<T>(
        &self,
        inspect: impl FnOnce(*mut std::ffi::c_void, *const {table}) -> T,
    ) -> T {{
        inspect(self.state(), self.{getter}())
    }}
'''
STATE = '''    fn state(&self) -> *mut std::ffi::c_void {
        unsafe { (*self.raw).state }
    }
'''
ADDITIONS = [
    ('src/sim.rs', STATE, NATIVE_CONTEXT.format(table='SimVtableV1', getter='sim')),
    ('src/ai_ctx.rs', STATE, NATIVE_CONTEXT.format(table='AiVtableV1', getter='table')),
    ('src/wrapper.rs',
     '        container.export.draft_hooks_len = container.draft_hooks.len();\n',
     '        container.export.item_build_hooks_ptr = container.item_build_hooks.as_ptr();\n'
     '        container.export.item_build_hooks_len = container.item_build_hooks.len();\n'),
]


def main():
    if not (SOURCE / 'mod-api-stable' / 'Cargo.toml').exists():
        sys.exit(f'Game SDK not found at {SOURCE}; set TFM2_GAME_DIR to the game folder.')
    if TARGET.exists():
        shutil.rmtree(TARGET)
    TARGET.mkdir()
    shutil.copytree(SOURCE / 'mod-api-stable', TARGET / 'mod-api-stable')
    shutil.copy2(SOURCE / 'base_version.txt', TARGET / 'base_version.txt')
    for name, anchor, addition in ADDITIONS:
        path = TARGET / 'mod-api-stable' / name
        text = path.read_text(encoding='utf-8')
        if addition.strip() in text:
            continue
        if text.count(anchor) != 1:
            sys.exit(f'{name}: anchor found {text.count(anchor)} times; the SDK changed, update tools/prepare_sdk.py')
        path.write_text(text.replace(anchor, anchor + addition), encoding='utf-8', newline='')
    version = (TARGET / 'base_version.txt').read_text(encoding='utf-8-sig').strip()
    print(f'Prepared SDK {version} in {TARGET} with the mod\'s local additions.')


if __name__ == '__main__':
    main()
