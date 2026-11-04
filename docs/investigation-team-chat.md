# Investigation: the 0.7.0 team chat as a base for a standalone mod

Written 2026-10-30. Question from the author: can the new team chat
(戰隊聊天, the "Messenger" tab) be used by a **standalone framework mod**
to make conversations with athletes more dynamic?

Every finding is labelled:

- **Seen in game**: the author played it (build and game version given).
- **Read from the code**: static reading of the 0.7.0 executable
  (sha256 `d678e012…7d30`), its data bundle (`ff41053c…3bae`) or the
  0.7.0 stable SDK (`mod-sdk-stable`, base 0.7.0).
- **Guess**: plausible, not tested.

Nothing here was tested in game. No code was written.

## 1. Short answer

- The chat is a real, data-driven system inside the save, but the
  game's official mod interface (the SDK) **cannot read or write it**.
- Through the SDK alone, a mod can run **its own** conversations next to
  the game's, react to matches and transfers, and change an athlete's
  morale, stress and loyalty. It cannot touch the game's trust bar,
  captain, promises, team atmosphere, the 3-talks-a-week limit, the
  thread list or its red unread dot.
- Writing into the game's own chat is possible only with a native hook
  (direct memory access), which is a brand-new system that will likely
  change every patch, and which puts the mod's text keys inside the
  player's save.
- The cleanest long-term answer is to ask the developer to add the chat
  to the SDK, which already grows by "ABI levels" (level 10 in 0.7.0).

## 2. What the chat is

**Seen in game** (author's screenshot, 0.7.0): a thread list (team
group, general manager, each athlete), an athlete thread with a trust
bar (信任, "普通"), an "appoint as captain" button, a salary-advance
conversation with three replies and a system line explaining the
repayment, and five coach-started topics (praise, encourage, advise,
talk role, check in) limited to 3 per week, one per athlete.

**Read from the code**: the whole state lives in one top-level part of
the save, `Database::squad_talk_state`, next to `teams`,
`rivalry_state` and `manager_career_state`. It is **not** a field of
the Team record (Team's 71 fields have nothing talk-related) or of the
Athlete record.

| Structure | Fields (from the executable's field tables) |
| --- | --- |
| `SquadTalkState` | `next_id`, `teams` |
| `TeamTalk` (one per team) | `team_id`, `week_key`, `week_talks`, `trust`, `threads`, `promises`, `cooldowns`, `group_event_dates`, `captain`, `captain_since`, `advances`, `atmosphere` |
| `TalkTrust` | `value`, `last_interaction`, `manager_talk_week` |
| `TalkThread` | `id`, `lines`, `unread`, `pending`, `updated_at`, `contact`, `athlete_id` |
| `TalkLine` | `at`, `key`, `binds`, `speaker` |
| `TalkPending` (a question waiting for the coach) | `topic`, `deadline`, `seen`, `alerted`, `expires`, `subjects`, `options` |
| `TalkOption` (one reply button) | `id`, `key`, `binds`, `replies`, `effects`, `promise` |
| `TalkPromise` | `id`, `kind`, `made`, `due`, `state`, `thread_id`, `athlete_id` |
| `TalkAdvance` | `granted`, `amount`, `remaining` (+1) |
| `TalkSpeaker` | `Athlete{athlete_id}` or `Executive{team_id}` |

`TalkEffect`, the list attached to each reply option, has these kinds:
`Trust{athlete_id, delta}`, `Athlete{athlete_id, stress, morale,
loyalty}`, `Atmosphere{delta}`, `Roster{stress, morale}`,
`SalaryAdvance{athlete_id, amount}`, and manager-career ones
(`CareerObjective`, `CareerOffer`, `CareerOfferTerms`,
`CareerRenewal`). Promise kinds include language lessons, champion
practice and champion chance.

What this means:

- **The save stores text keys, not text.** A line is a key such as
  `talk.t11.open.0` plus filled-in values (`binds`, e.g. `{Weeks}`,
  `{AmountMoney}`); the words come from the text sheet
  `asset/base/text/news` at display time, in 17 languages.
- **Each reply carries its own consequences as data.** The game applies
  the option's effect list when the coach picks it. A pending
  conversation is therefore a self-contained package: lines, options,
  effects, an optional promise.
- **The topics are fixed in code.** Text keys show topics `t01`–`t11`
  (athlete-started: bench time, milestones, salary advance, …),
  `g01`–`g10` (group chat: welcome a newcomer, blame after a loss with a
  captain option, …), `praise`, `encourage`, `advice`, `role`,
  `checkin`, `captain`, and `board`/`club` (manager career). The
  decision logic is in `game-view\src\logic\system\squad_talk.rs`.
- **The screen only shows; the management server decides.** The screen
  sends messages `TalkStart`, `TalkReply`, `TalkSeen` and
  `TalkSetCaptain` to the management server, which owns the state. Any
  change that must last has to happen on the server side.
- The Messenger screen is its own native screen type (`messenger_ui`),
  layout `asset/base/ui/layout/messenger`, with pieces in
  `messenger_component/` (`thread_row`, `line_athlete`, `line_manager`,
  `line_system`, `option_button`, `topic_button`, …).
- Separate system, same word: the in-match chat bubbles
  (`asset/base/text/chat`, `chat_pipeline.rs`) belong to the 0.7.0 match
  AI, not to the Messenger. Not covered here; see the pings work.

## 3. What the official interface (SDK) reaches

**Read from the code** (0.7.0 SDK):

- A mod has two halves. The **server half** runs inside the management
  server with hooks before/after each management tick and a command
  handler. The **screen half** runs in the client. They talk through
  `send_command` (screen → server) and `emit_event` (server → screen).
- Server half can:
  - read and write Team and Athlete records as JSON
    (`team_get_json`/`athlete_set_json`; writes are checked and refused
    whole if invalid);
  - read and write other records: League, Tournament, Staff, matches,
    schedule (`RecordKindV1`); match replays are read-only;
  - keep its own data inside the save, in its own namespace
    (`save_get`/`save_set`);
  - read management events: `MatchFinished`, `TransferCompleted`,
    `SeasonRollover`;
  - push a news article to a team's feed (`news_push`);
  - force a transfer.
- Screen half can: add UI from the game's own `.ui` templates or from
  `.ui` source under any node path, register click handlers, read and
  set label text, play sounds, know which main tab is open
  (`client_main_tab`), read records read-only, draw directly.
- The Athlete record has a `management` field whose type
  (`AthleteManagementStat`) holds `stamina`, `condition`, `stress`,
  `loyalty`, `morale`, `fan_satisfaction`, `fan_count`,
  `fan_expectation`. So morale, stress and loyalty should be readable
  and writable through `athlete_set_json(id, "management.morale", …)`.
  **Guess** until tested: that a write sticks and the game does not
  immediately recompute it.
- **Not reachable**: `squad_talk_state`. There is no talk record kind,
  the SDK contract has no "talk", "trust" or "squad" entry, and the
  server docs say direct Database changes are deliberately excluded.
  Trust, captain, promises, atmosphere, weekly talk count, threads,
  pending questions and the unread dot are all out of reach.

## 4. Routes

### A. Text and layout only (data mod, no code)

The game's override file (`mod.override_info`) can **merge** keys into
a text sheet and **replace** a UI layout; the installed Riot items mod
does both for `text/item`, `text/ui` and several layouts (**read from
the code**: its files).

- Reaches: rewording or translating existing chat lines; restyling the
  Messenger layout (node names must stay, the game's code finds nodes by
  name).
- Cannot: new topics, new triggers, new effects.
- Open: adding extra variants (e.g. `talk.t01.open.2`) works only if the
  game counts the variants rather than using a fixed number. **Guess**.
- Upkeep: low. Save risk: none.
- Value for "more dynamic": small.

### B. A framework on the official interface only

The mod runs its own conversation engine:

- Topics as data files (content packs): a trigger (match won/lost, a
  milestone in match records, low morale, contract ending, transfer
  arrived, new season, a random check-in), lines as text keys merged
  into a text sheet of the mod's own, reply options, and effects limited
  to what the SDK can apply (morale/stress/loyalty, news, money through
  Team fields, the mod's own "relationship" values in its save data).
- State (threads, cooldowns, its own trust-like value) in the mod's
  save namespace, so removing the mod leaves the game's save valid.
- Shown in one of three ways: (1) a panel on the Messenger tab built
  from the game's own `messenger_component` templates so it looks
  native; (2) a small mod button that opens its own chat window; (3)
  news articles for one-way messages.
- Other modders add content by writing data, not code. This fits the
  "clean base for other modders" goal.
- Cannot: share the game's trust bar, captain, promises, atmosphere or
  thread list; the player sees two systems side by side. Can only
  approximate trust with its own value.
- Upkeep: low. A stable-SDK mod keeps loading on future versions unless
  the game drops a slot. Requires base `>=0.7.0` only if it uses the
  Messenger tab; otherwise it would run on 0.6.5 too.
- Save risk: low (own namespace; athlete writes are validated).

### C. A native hook into the game's chat

Read and write `squad_talk_state` directly from the server thread, so
the mod's conversations appear as real threads with real options, and
the game itself applies the effects (trust, atmosphere, promises
included).

- Reaches: everything the native chat does.
- Needs: the Database's location from the server side, the offset of
  `squad_talk_state`, and either the game's own (de)serialisation code
  for these types or the raw layout of Rust vectors and strings, with
  the game's allocator. Whether the game has a JSON converter for
  `SquadTalkState` compiled in is unknown (**Guess**; the save uses a
  binary format).
- Upkeep: high. A system new in 0.7.0 is likely to change in the next
  patches; every patch means finding it again (version folders,
  fingerprint, layout guards as in AGENTS.md).
- Save risk: high. Lines store text keys, so a save keeps pointing at
  the mod's keys after the mod is removed (lines would show raw keys or
  empty text); a wrong write corrupts the part of the save the game
  loads on every start.

### D. Ask the developer

Request for the SDK: a talk record kind (read/write JSON, like Team),
or at least slots to push a pending conversation with options and the
existing effect kinds, plus a management event when the coach replies.

- The SDK has grown before: its notes mark slots added at levels 2, 3,
  4, 6 and 10 (for example `MatchReplay` records at level 3 and
  `AutoPatch` settings at level 10). Whether those came from modder
  requests is not known.
- Reaches: what C reaches, with no upkeep, if accepted.
- Cost: waiting; may be refused.

## 5. Open questions and the test that answers each

| # | Question | Test (no new code unless said) |
| --- | --- | --- |
| 1 | Live UI path of the Messenger screen; whether the game rebuilds it and wipes added nodes | Enable the installed LT Athlete Probe, open the Messenger tab, press F12; the dump goes to `%LOCALAPPDATA%\LTTools\lt_athlete_probe\`. The "wipe" part needs a small test mod (code, needs a go-ahead). |
| 2 | Does `athlete_get_json(id, "management")` return morale/stress/loyalty, and does a write stick? | Read: the probe's F10 dumps whole athlete records. Write: a small test mod (needs a go-ahead). |
| 3 | What do the `MatchFinished` / `TransferCompleted` / `SeasonRollover` payloads hold? | Not documented in the SDK. A small test mod that logs them (needs a go-ahead). |
| 4 | Does a merged extra line variant get picked? | Data-only test mod adding `talk.t08.open.*` variants to `text/news`; play until a milestone talk. Data only, but still a build: needs a go-ahead. |
| 5 | Is there a JSON converter for the talk types in the executable (makes route C cleaner)? | Static reading, no game run. |

## 6. Fit with the project direction

- The mod's direction is playing one athlete. The chat is the coach's
  side. A standalone chat framework is a separate mod, not part of LT
  Direct Control.
- Possible tie-in later: in a career-mode save the player *is* the
  athlete, so the same engine could run the other way (the coach and
  teammates message you). Not investigated.
- A language-model writer for lines is possible in a code mod but adds
  cost, delay, 17-language output and content-control questions; not
  investigated.
