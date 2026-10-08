# Backlog

Reported by the user; to do after the codebase clean-up (stages 5–6 of
[codebase-review-66.md](codebase-review-66.md)).

1. **Ability tooltip numbers show "…".** Example (2026-10-09, 0.66.1): the
   Illusionist's R ("Copy": an ally illusion lasting … seconds) and Q
   ("charm bolt": 60 + …% magic attack, taunt for … seconds). Seen often,
   across many champions. Start in `probe/src/tooltips.rs`; its doc says
   unknown formula parameters stay visibly unknown, so these values are
   probably not found in the data it reads.
2. **Done in 0.67.0.** **F11 should also pause.** Today F11 starts or resumes control; pressing
   it while running should pause, so one key toggles.
3. **Done in 0.67.0 (Vanilla order, default on).** **Shop design: queued items buy small components.** With several items
   queued, each affordable next step is bought at once, so slots fill with
   cheap components instead of saving gold for a bigger upgrade. User
   decision (2026-10-09): offer "first order first" as an option. That is
   the vanilla buying behaviour (finish one item before starting the next):
   only the first unfinished order buys; later orders wait.
4. **Won't do (user, 2026-10-09: too much work for a small thing).** **Esc on the shop also opens the game's Esc menu.** While the shop or
   the mod's Settings window is open, Esc should only close that window.
   Otherwise Esc must keep opening the game's menu (quit, game settings).
   Finding (0.67): the spectator-key hook already suppresses Esc during
   control (game key code 0x37; the game's key table is Left, Right, Up,
   Down, Tab, A-Z, Space, LShift, LCtrl, Num1-8, F1-F12, Enter, Escape, ...),
   yet the menu still opens, so the game opens it through another path. The
   SDK cannot consume keys. Fixing it needs that path found and hooked:
   a reverse-engineering task.
5. **Skill previews are wrong for many skills.** The aim/range previews
   (`probe/src/skill_preview.rs`, `native_preview.rs`, `preview_assets.json`)
   do not match the real skill for a large number of champions, and fixing
   them one by one by hand is not realistic. Needs a dedicated focus session:
   find a general way to derive each preview from the game's own skill data.
