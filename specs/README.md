# Specs

Behavior worked out from reference material on other games enters this
repository only as specs in this folder, written in our own words.
Implementation is written from the specs alone. No code from a source outside [THIRD_PARTY.md](../THIRD_PARTY.md), and no assets, are ever committed. See
"Assets and legal" in the top-level README for why.

## Who does what

- **Spec sessions** work from the reference material the user points them
  to (for CS:S, also Valve's public Source SDK 2013) and write specs here.
  They run where that material is (e.g. a Claude Code session on the
  Windows PC, in a checkout of this repo). They never write implementation
  code.
- **A human reviews** every spec before it is implemented.
- **Implementation sessions** see only `specs/` and our code, never the
  reference material. They turn each spec's test cases into automated
  tests first.

Keeping reading and writing in separate sessions is what keeps the result a
reimplementation and not a translation.

## Rules for spec writers

Never commit or paste:

- Code, pseudocode that mirrors it line by line, or the reference's
  function/class/file structure. Describe *what happens*, organized by
  behavior, not by where it lives in the original.
- Identifier names, comments or strings copied from the reference, except where
  the name is itself data the game exposes (a weapon ID in a data file, a
  console variable a player can type).
- Encryption keys, credentials, server addresses or anything secret.
- Game assets or extracted files.

Do capture, precisely:

- Every constant, with units and the game's coordinate convention.
- Formulas as math.
- Order of operations within a tick. Feel lives here.
- Tick rate, and anything that depends on frame rate.
- Clamps, rounding, edge cases.
- Quirks and bugs players rely on, stated deliberately.

If the reference is ambiguous or you are not sure, say so in "Open questions"
instead of guessing.

## Layout

```
specs/<game>/<topic>.md      e.g. specs/cs_source/movement.md
```

Combat Arms specs (`specs/combat_arms/`) are written on the Windows PC and
pushed to main, one file per session; the plan and the spec
briefs are in
[docs/plans/active/combat-arms.md](../docs/plans/active/combat-arms.md).

## Template

```markdown
# <Game>: <topic>

Source basis: <what the spec was worked out from, described generally by
role, e.g. "the client's player movement and shared physics", no paths>
Status: draft | reviewed | implemented

## Summary
Two or three sentences: what this system does and what makes it feel the way
it does.

## Units and conventions
Distance unit, up axis, angle units, time base (ticks/seconds), tick rate.

## Constants
| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|

## Behavior
Prose and math, organized by behavior (e.g. "Ground acceleration",
"Jumping", "Sprint and stamina"). For each:
- inputs and state it reads
- the formula
- what it writes

## Per-tick order
Numbered list of the steps in one tick, in order.

## Edge cases
Slopes, steps, landing, collisions, interactions with other systems.

## Quirks
Behavior that looks like a bug but is part of the feel, and whether to keep it.

## Test cases
Worked out from the reference. Each one becomes an automated test.
| Setup | Input | After | Expected |
|---|---|---|---|
| standing still on flat ground | hold forward | 1 tick | speed = ... |

## Open questions
Anything ambiguous, and how it could be checked against the live game.
```

## Prompt for a spec session

Start Claude Code in a checkout of this repo on the machine that has the
reference material, then give it:

> Read specs/README.md and follow its rules exactly. Write
> specs/<game>/<topic>.md from the reference material at <path>.
> Do not copy code, identifier names or comments into the spec. Describe
> behavior as constants, math, per-tick order, edge cases, quirks and test
> cases with concrete expected numbers. Do not write implementation code.
> Do not commit anything outside specs/.
