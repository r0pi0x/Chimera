# Task briefs

Self-contained tasks a contributor's coding agent can pick up, one per
file, in the shape of
[.github/ISSUE_TEMPLATE/agent-task.md](../../.github/ISSUE_TEMPLATE/agent-task.md).
Drafts: not yet posted as issues. Rules for every task: [AGENTS.md](../../AGENTS.md).

| Brief | Size | Needs install |
|---|---|---|
| [01 Key the single-slot net and core hooks by game](01-game-hooks-keyed-by-game.md) (done) | M | no |
| [02 Run the ladder's per-tick update as systems](02-ladder-update-as-systems.md) (done) | S | no |
| [03 Remove map-killed weapons without polling the world](03-removed-map-weapons.md) (done) | S | no |
| [04 Minigame runtime's per-tick work as normal systems](04-minigame-runtime-systems.md) (done) | S-M | no |
| [05 Rules modules' join and arming as entity events](05-joined-arming-events.md) | S | no |
| [06 Round and match lifecycle as a triggered event](06-mode-step-event.md) (done) | S-M | no |
| [07 Map-load progress as a resource, not a static](07-load-progress-resource.md) | S | no |
| [08 Default binds per game as a resource](08-bind-defaults-resource.md) | S | no |
| [09 Console map names and map cycle per game, as a resource](09-console-map-names.md) | S | no |
| [10 Fewer archetype tables for characters](10-fewer-character-tables.md) | M | no |
| [11 Heavy tier: one more slot, then a converted-map cache](11-heavy-tier-time.md) (slots and cache design done; cache left) | S + L | cs_source |

01-06 and 07-09 are steps of
[plans/active/ecs-cleanup.md](../plans/active/ecs-cleanup.md) (section 4);
read its section for the step before starting, and mark the step done
there in the same PR.
