# Counter-Strike 1.6: movement

Source basis: <by role, e.g. "measurements of the running game", "the
game's movement settings as the console lists them"; no paths>
Status: draft

## Summary

## Units and conventions
Distance unit, up axis, angle units, time base (ticks/seconds), tick rate.

## Constants
| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| walk speed | | units/s | |
| gravity | | units/s² | |
| jump | | | |

## Behavior
By behavior (ground acceleration, friction, air control, jumping,
crouching, steps, slopes): what it reads, the formula, what it writes.

## Per-tick order

## Edge cases

## Quirks

## Test cases
| Setup | Input | After | Expected |
|---|---|---|---|
| standing still on flat ground | hold forward | 1 s | speed = ... |

## Open questions
