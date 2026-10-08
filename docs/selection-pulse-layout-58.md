# Selection pulse and spectator layout lease (0.58.0)

The user requested a thinner attack-target outline and the priority attack click
> hover > attack target. They also reported that opening the full spectator detail
panel after returning control to AI left later direct-control matches confined to
the left side. This pass covers both; in-game confirmation remains pending.

## Outline policy

Attack offset changes from 1.0 to 2/3 sprite-coordinate units. Hover retains the
exact 1.5 crisp / 2.5 wide offsets and team colours. A valid explicit Attack order
from right-click or armed A-left-click records the enemy ID and monotonic click
time. Held inputs, worker ticks and AttackMove auto-acquisition do not record one.
Another accepted click on the same enemy restarts it; invalid UI clicks do not.

The client captures the click stamp alongside its existing hover/attack frame
feedback. The stamp is discarded when no longer the current valid attack, stale,
hidden/dead, inactive, or older than 120 ms. The native wrapper checks the same
attack ID again, interpolates offset from 3.0 to the applicable settled offset,
and selects exactly one role for that body. After 120 ms, hover wins if present;
otherwise the thin attack outline remains. Hover elsewhere can coexist as before.
This is our approximation of the user's League observation, not a claim of an
identical League shader or timing.

No new hooks, layers or render passes are added. Existing four independent sprite
silhouette passes, command cleanup, fallback and identity/position guards remain.
The outline setting gates all roles and the debug marker setting remains separate.
`click_drawn` joins the existing per-role renderer-call counters.

## Spectator layout

In game 0.6.3 the battlefield renderer reads config byte +0x45 at RVA 0x23228d4;
nonzero chooses full width, zero chooses compact geometry, with +0x46 controlling
compact-side placement at 0x2322906. Existing camera-frame reconstruction already
used those flags. Hiding spectator panels in TeamInfo did not change them, so a
persisting compact preference left a narrow native battlefield behind the HUD.

CameraLease now captures the original +0x45 byte with the original camera union
and vision. It binds to the exact live viewer/config pair before the first owned
viewport read, including bootstrap. While owned, +0x45 is forced to 1 before native
view update and reasserted before camera capture/render. Reassertion never replaces
the saved original. Releasing control restores the original flag, camera and
vision only for that same live pair; stale addresses are not dereferenced. Session
reset discards old leases, so a fresh viewer captures its own spectator preference.
The compact-side flag, zoom, and native minimap positioning remain unchanged.
Existing TeamInfo suppression/restoration handles the associated spectator UI.

The two field consumers are added to the reviewed profile, runtime byte checks
and source/profile verifier. The exact game executable identity remains mandatory;
there is no disk executable edit or runtime search. A changed native profile is
checked explicitly by the migration workflow before future game support.

## Verification

255 probe + 18 core tests pass, including monotonic one-shot/repeated-click input,
auto-acquisition exclusion, expiration, fog/stop/inactive/death/reset clearing,
all three role priorities and interpolation, exact config restoration, rejection
of different viewer/config pointers and independent new-viewer preferences.
Clippy with warnings denied, formatting, 0.6.3 profile checks (including the two
new layout consumers), all 15 outline CALLs and full native cleanup verification
pass. Native layout, appearance and performance require `TESTING-58.md`.
