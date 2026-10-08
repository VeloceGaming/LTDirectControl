# Session controls, probe 0.23

The native UI subtree `ingame.lt_session_controls` provides Start control in
READY, Pause while running, Resume while paused, and Return to AI in all three.
Ctrl+Home starts/resumes as a backup; Ctrl+End remains emergency release.
Return to AI is terminal for the current battle. In 0.23 the client rearms after
release outside the battlefield, on Main/Lineup/StadiumEntrance/Match, or on a
return to title. Result, SetFeedback and replay screens do not rearm control.
The next foreground worker must start at tick 1 and repeat the published-frame,
bootstrap and pause acknowledgement checks before offering Start control.

Rearm clears worker/sender/view bindings, counters, pending UI actions and every
command/actor/HUD/targeting/camera consumer. The chosen lane persists as a default,
but selection is unlocked and ownership/roster is read again. Old publication
waits carry a generation and cannot wait on or cancel a later session. Retired
match keys block late same-battle analysis callbacks; title clears that set so
the same saved battle can be played again. Title/result/feedback scenes close
the binding window. Installation failure remains terminal; hooks install once.

A short shared gate excludes concurrent SDK scalar writes during the consumer
reset. No host API call or pacing wait occurs under the exclusive gate. Camera
leases from a finished viewer are forgotten without dereferencing old addresses;
live release restoration remains inside the current viewer borrow. Physical key
histories are seeded on reset so held inputs cannot become fresh presses.

Literal work: read local stable SDK widget spawning, path-event registration,
current-event and record-query APIs. Event callbacks queue only an action bound
to the current match key and phase. Client post-update rejects stale requests,
clears pending movement/cast/recall commands and applies accepted transitions.
Physical input histories continue updating while gameplay is disabled so held
inputs cannot become new orders on resume. Session widget bounds block both
battlefield commands and camera panning. Champion-only mode persists on pause.

Pause reuses existing native publication/view coordination. The worker waits
after publication, outside the simulation locks; the bound viewer clears its
playback accumulator and receives zero playback delta. Camera updates use the
original client delta. No waits occur in SDK/UI callbacks. Up to two frames
already generated in flight can remain queued; Resume continues from that
queue. Manual actor ownership remains held through a possible in-flight tick.
Already committed native actions/channels are not reset by clearing commands.

Running has no 60-second limit. Startup interception still expires at 15 seconds,
READY without Start at 120 seconds, and a missing client heartbeat after READY
at two seconds, including while paused. Identity/thread/view mismatches and
battlefield exit still release coordination. Pause does not count as READY
waiting. No native branch was added or changed for these controls.

Read-only result capture uses SDK history queries. At binding it records replay
IDs; for up to 30 seconds after battlefield exit it samples history at most once
per second. It writes changed evidence to
`research/session-result-<match_id>-set<set_index>-seed<seed>-run<generation>-<version>.json`, including the last
same-match selected-player sample, same-numeric-ID records from available match
categories, and up to eight new/referenced replay candidates. Category IDs can
collide and the last player sample need not be the terminal tick. Candidate
records are explicitly UNVERIFIED; winner/stat/replay comparison must establish
their relationship to the played battle. No history, save or outcome is changed.

0.21 restricts player samples to the original worker (including after Return to
AI while the battlefield remains open) and freezes that sample at battlefield
exit. Later analysis/re-simulation workers with the same key cannot overwrite
it. A bounded read-only walk of main.contents on MatchResult also captures text
labels for comparing the displayed result. The history polling window remains
30 seconds; no replay record in that window means saved authority is unverified.

0.23 retains the prior audit while armed between battles, replacing it when a
new foreground key/generation is bound. Title clears the audit to avoid reading
another save's tables into it. Seed/generation filenames preserve repeated runs.
The log's bounded sampling budget restarts per session, with the file appended.

The 0.23 user test reports success; the recording confirms three controlled
sets in one process, including the own team moving from red to blue. The final
match-33 record links replays 882/883/884. Their seeds, final ticks, champion,
athlete/player and KDA match those original workers. The match record ends with
winner team 7, loser team 3, team1 score 1 and team2 score 2. This establishes
recorded series authority for that 2–1 win; no disk save reload persistence test
was performed. Generic captures remain marked UNVERIFIED until compared.
Evidence: `research/session-result-33-set3-seed14400468899550163220-run2-0.23.0.json`
and `research/probe-2026-10-02-twenty-third.log`.
