# Third-party notices

The mod's own code, artwork and documents are released into the public domain
(see [LICENSE](LICENSE)). The parts below come from others and keep their own
licenses. Their conditions apply to those files only.

| Part | Where | License | What it asks |
|---|---|---|---|
| Manrope font | `probe/font/manrope_*.ttf` (packaged); design previews | SIL Open Font License 1.1 ([text](probe/font/Manrope-OFL.txt)) | Keep the license text with the font; do not sell the font on its own |
| Noto Sans TC font | `probe/font/noto_tc_medium.ttf` (packaged); design previews | SIL Open Font License 1.1, Reserved Font Name "Source" ([text](probe/font/NotoSansTC-OFL.txt)) | As above; a modified font may not be called "Source" |
| Archivo, Iosevka fonts | `design/hud/settings-source/src/fonts` (design previews only) | SIL Open Font License 1.1 | As above |
| Shop header stamp | `probe/ui/nerdge_stamp.png` and its source `Nerdge_Stamp_Transparent.png` | Drawn by the mod's author after the community "Nerdge" meme emote, whose original creator is unknown. **Not** covered by the public-domain dedication; the author keeps their rights | Not for reuse outside this mod. If it must go, the shop header falls back to a plain title |
| Lucide icons | the white UI glyphs in `probe/ui` and `design/hud/endfield-assets/icons` | ISC ([text](probe/ui/Endfield-icons-LICENSE.txt)) | Keep the copyright and permission notice |
| Teamfight Manager 2 mod SDK | not in this repository; `tools/prepare_sdk.py` copies it from the game into `sdk/` (ignored by git) | No license file; belongs to the game's developer | Not covered by this project's license and not redistributed. Use the SDK that comes with the game |

The packaged font files are static instances generated from the original
fonts (`tools/generate_hud_fonts.py`); the OFL permits this.

The packaged mod contains no game artwork, sprites or code. It embeds some
game data as plain numbers and names (skill parameters, sprite sizes, asset
paths in `probe/src/*.json`), read from the installed game.

Not for public release (design references only, owned by others):
`design/*/game.webp` (a game screenshot used behind mockups),
`design/hud/shop-assets/item-icons.png` and `riot-text.json` (item icons and
text from the game and the Riot item mod), and the game's mod SDK.
