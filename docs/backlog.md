# Backlog

Reported by the user; to do after the codebase clean-up (stages 5–6 of
[codebase-review-66.md](codebase-review-66.md)).

1. **Ability tooltip numbers show "…".** Example (2026-10-09, 0.66.1): the
   Illusionist's R ("Copy": an ally illusion lasting … seconds) and Q
   ("charm bolt": 60 + …% magic attack, taunt for … seconds). Seen often,
   across many champions. Start in `probe/src/tooltips.rs`; its doc says
   unknown formula parameters stay visibly unknown, so these values are
   probably not found in the data it reads.
2. **F11 should also pause.** Today F11 starts or resumes control; pressing
   it while running should pause, so one key toggles.
3. **Shop design: queued items buy small components.** With several items
   queued, each affordable next step is bought at once, so slots fill with
   cheap components instead of saving gold for a bigger upgrade. Needs a
   decision on the rule (for example: finish the first order before starting
   another, or only buy when a whole step of the first order is affordable).
