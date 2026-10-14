# Investigation: does the played match count?

Started 2026-10-14 (mod 1.2.3, game 0.7.0) after the author noticed that
an athlete's season record did not move by what he had just played.

Short answer: **partly.** The match record and its replay follow the match
the player controlled. The athlete's season totals do not; they follow a
second, AI-only simulation of the same game.

Each finding says what kind of claim it is (see `AGENTS.md`).

## What the game does

**Read from the SDK and the official documentation.** A fixture is
simulated more than once. `SimOriginKindV1` names where a simulation runs:
`ServerPresim` ("server pre-simulation of the same fixture") and
`ClientMatchView` (the match on screen), among others. The documentation
requires every simulation callback, player AI included, to be
deterministic, so that both runs give the same result. A mod that lets a
person play breaks that assumption by design: the mod only takes control in
`ClientMatchView`.

**Seen in the mod's own logs** (six fixtures in the kept `probe-*.log`
files, game 0.6.2 and 0.6.3, verbose `SIM` lines). For the same seed and
match id:

- a `kind=1` (pre-simulation) run and the `kind=2` (watched) run start
  within about 10 ms of each other;
- the pre-simulation finishes the whole game in about 5 seconds (for
  example 31,200 ticks in 4.6 s), while the watched match takes minutes;
- they end at different ticks (37,200 against at least 31,800 in one case).

So the pre-simulation's result exists a few seconds after the match starts,
long before the player has finished playing.

## What follows which simulation

**Seen in the game's own data** (author's save, 2026-10-14; athlete 599,
ten games, all played by the author). The stored replay records were read
two ways, by the mod's `result_audit` right after each match and by the
author's LT AI Coach exporter (`match_replays.json`), and agree.

| | Sum of the ten stored replays | Season record screen and league ranking |
| --- | --- | --- |
| Kills / deaths / assists | 143 / 12 / 30 | 96 / 19 / 57 |
| Wins - losses | 10 - 0 | 9 - 1 |
| MVP | 10 | 6 |
| Solo kills | 83 | 25 |
| Average level | 15.2 | 13.40 |

- The stored replays hold the played result: each game's length equals the
  ticks the mod played, and the numbers equal the result screen.
- The match record of the last series says 2-0 for the author's team, as
  played.
- The season totals cannot be the sum of those replays. They read like an
  AI's games, including one loss in a game the author won.

**Guess, well supported:** the season totals are accumulated from the
pre-simulation. It has not been observed directly: no log yet holds a
pre-simulation's final numbers next to the season record. *Superseded
2026-10-14 by the diagnostic below: observed.*

**Superseded 2026-10-14:** `docs/history/control-pass-27.md` says the result
audit was "verified read-only against four game records". That compared the
stored replay records only, never the season totals, and does not show that
the played result counts.

## Diagnostic result (2026-10-14, 1.2.3, game 0.7.0)

**Seen in game** (author's series, match 616, two games, both won 2-0 in
play; records read through the official interface by the 1.2.3 diagnostic
at the start of game 2, one minute in, on leaving and 30 s later).

The athlete record has `season_statistics[].by_position.<lane>`: matches,
wins, kills, deaths, assists, mvp, solo_kill, solo_killed, level, rating,
dealing, tanking, healing, gold, `champion_detail`, `played_champion`. The
league competition record (`LeagueCompetition` 6, the author's league) has
`standings.<team>` (win, lose, set_win, set_lose, kill, death, assist) and
`statistics.<athlete>` with the same fields as the athlete's.

| What | Changed when | Follows |
| --- | --- | --- |
| Athlete `season_statistics` | between 0 s and 60 s into game 2, while it was still being played | the pre-simulation, exactly: all five own athletes moved by their pre-simulation K/D/A and level (Sunna +1/+2/+3, level +11; played 12/3/7, MVP) |
| League `statistics.<athlete>` (season record screen, player ranking) | after the series ended | a copy of the athlete's `season_statistics` |
| League `standings.<team>` | after the series ended | the played games: team 65 kills +37, deaths +16, assists +53 = the two stored replays; set wins +2 |
| Match record, stored replays | as before | the played games |

The pre-simulation of game 2 had finished its 32,160 ticks within the
first minute. Its final K/D/A per player were sampled from inside it.

So the series result and the team table follow the played match. Every
per-athlete statistic (K/D/A, MVP, rating, solo kills, level, wins and
matches counted per athlete) follows the AI-only pre-simulation, applied a
few seconds into the match.

Loose ends, not explained: no `start` snapshot was written for game 1 (the
own roster was not yet known while the battlefield was first shown); after
leaving game 2 another `ClientMatchView` run of the same key reached tick
30,780 with the pre-simulation's numbers.

## First correction (1.2.3, series 620, 2026-10-14)

**Seen in game.** The stored replays of a series appear only when the
series ends (game 1's replay did not exist while game 2 was played), and
the league's per-athlete statistics move at that same moment, to the
athlete records as they stand then. A correction made per game therefore
lost game 1 (still the pre-simulation's 4/4/6, a loss, against 14/2/5
played and won) and the league kept both pre-simulated games: Sunna's
season record grew by 9 kills for 26 played. The athlete write itself
worked: sent, accepted by the server, and seen by the client about 13 s
later. The correction is now made once per series.

## Per-series correction and repair (1.2.3, 2026-10-14)

**Seen in game.** Series 624 was corrected per series: all ten athletes'
records and league statistics were written and seen by the client; the
author's season screen matched his games, and the numbers stayed after a
save and reload.

**Seen in game.** The repair of the past checked 1,156 records of athletes
who never met the author's team against their stored replays (all equal),
then rebuilt 82 records (41 athletes, athlete record and league). The
client's copy showed the new values only after the next time skip, not
within 15 s. Sunna's season went from 138/29/89, 9 MVP, 14 wins (the league
copy) to 218/19/59, 16 MVP, 16 wins in 16 games.

## Not known yet

- Where the season totals are stored (the athlete's record, the league
  competition's record, or both), and what the league ranking screen reads.
- What else follows the pre-simulation: athlete growth, champion
  proficiency, awards, team statistics, standings. The series result
  follows the played match in the one case looked at, but a series where
  the two simulations disagree on the winner has not been examined.
- Whether a record edit made on the server reaches the client's screens
  without a reload.

## What a mod can do about it

**Read from the SDK.**

- The mod's `think` callback already runs inside the pre-simulation, for
  every player, so the mod can keep each player's final numbers from both
  runs (`StableSim` player getters, `statistics_json`) with no native code.
- A server extension (`StableServerExtension`) can read and write athlete
  records and any record kind (`athlete_set_json`, `record_set_json`),
  receives a `MatchFinished` event for every match, can keep its own data
  in the save, and can take commands from the client. Writes are checked
  against the game's schema and rejected whole if invalid; keeping records
  consistent with each other is the mod's job.

Options, none built:

1. **Correct the totals after the match (official interface).** Record both
   runs' final numbers; when the match finishes, add the difference
   (played minus pre-simulated) to wherever the season totals live, for all
   ten players. No hook, no address. Needs the record layout first, and
   care with everything derived from the totals.
2. **Native: make the game take the watched match's result.** A new hook
   family to find again on every patch. Only worth it if option 1 cannot
   reach something that matters.
3. **Do nothing and say so** in the Workshop description.

## Next step

The fix in option 1 can now be specific. After each played game, for each
of the ten athletes: remove the pre-simulation's contribution from
`season_statistics` (the difference between the record just before the
pre-simulation is applied and just after) and add the played game's
`statistics` from the stored replay. The client side of the mod sees both;
only a server extension may write records, so the client sends the
correction with `send_command` and the server extension applies it with
`athlete_set_json`. The league's per-athlete statistics are copied from the
athlete records when the series ends, so they should follow. Open points:
taking the "before" snapshot safely ahead of the pre-simulation, athletes
of the other team, MVP and rating, and what happens if the game is left
mid-match.
