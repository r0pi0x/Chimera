# TF2 weapons: every player weapon, what is in, what is left

Status: batches 0 and 1 built, batch 2's first five families built (2026-11-23; section 4); the rest of batch 2 and batch 3 planned.
Inventory by class and slot, then a phased build order.

Sources: the installed game's `scripts/items/items_game.txt` (read locally
to build the inventory; **never copied into the repo**), `games/tf2/data/weapons/`
(what is in), `specs/tf2/weapons*.md`, `weapon_scripts.md` (what is
specified), docs/plans/active/tf2.md and tf2-parity.md.

## 1. Counts

- items_game has 522 weapon-slot entries (primary/secondary/melee/pda/
  building, plus the wearable slot items below). Most are skins.
- **Distinct entries in this table (shared guns counted once): 160**, of
  them 6 passive wearables (rows marked W), 2 MvM-only, 1 mechanic row
  (Airblast). Batches: 31 in batch 1, 68 in batch 2, 40 in batch 3. Skins grouped, not listed: Festive,
  Botkiller (Silver/Gold, Mk.II, Rust, Blood, Carbonado, Diamond),
  Australium, Strange, Genuine, Vintage, Collector's, the 15xxx "war paint"
  families (`teufort_`, `powerhouse_`, `harvest_`, `gentlemanne_`,
  `warbird_`, `pyroland_`, `concealedkiller_`), TTG/Poker Night, Promo and
  Upgradeable copies, Saxxy-class melee (Frying Pan, Conscientious
  Objector, Freedom Staff, Bat Outta Hell, Memory Maker, Ham Shank,
  Crossing Guard, Necro Smasher, Prinny Machete: one stat line each, a skin
  of the class melee for gun-game use). One entry = one behaviour; each
  row's skins ride on it for free (model/skin choice, `Strange` ignored).
- **In game: 21 data files = the 21 stock weapons** (Scattergun, Pistol,
  Bat; Rocket Launcher, Shotgun, Shovel; Flame Thrower, Fire Axe; Grenade
  Launcher, Stickybomb Launcher, Bottle; Minigun, Fists; Wrench; Syringe
  Gun, Bonesaw; Sniper Rifle, SMG, Kukri; Revolver, Knife). The Shotgun and
  Pistol are each one file for all classes that carry them. **0 unlocks.**
  Stock items not in: Medigun, Sapper, Disguise Kit, Invis Watch, Build and
  Destroy PDAs, the builder (toolbox).
- **Specified: the same 21** (weapons.md, weapons_more.md,
  weapons_stock.md, weapon_scripts.md). Partly: Medigun (one line, "heals,
  builds Uber, no damage"), airblast (reference only), Eyelander-type
  taunt voices. **Unlocks: none specified.**
- So 139 entries are not in game and have no spec of their own; 4 more
  are partly covered (Medigun, Airblast, Invis Watch, the PDAs). Which need
  a new spec first is in 6.

## 2. What exists to build on

Mechanics already in `games/tf2/src/game/weapons/`: single/multi-pellet
hitscan with distance falloff (`damage`), rockets (`rocket`), pipes
(`grenade`), stickies (`sticky`), shared blast (`blast`), flame particles
and afterburn (`flame`), the winding minigun (`minigun`), zoom/charge
sniper (`sniper`), needles (`needle`), melee with delayed swing and
backstab (`melee`), crits and crit boost (`crit_boost`), recoil, looks.
Per-gun numbers come from the game's own `tf_weapon_*.ctx` scripts at run
time (`script_numbers`) over the spec's numbers as fallback.

The item-attribute layer is batch 0 (below, built): a host-side
converter into the asset pack, the game reading the pack.
Not every behaviour is in attributes: e.g. the Backburner, Phlogistinator
and Manmelter carry no or few attributes and are coded by item class/id in
the game; those need a spec (clean-room measurement), not just a data
read.

## 3. Inventory

Columns: **In** = data file exists. **Spec**: S specified, P partly, N none.
**Family** (reused mechanic): HS hitscan, PRJ projectile, FT flame, MEL
melee, STK sticky, MED medigun, BUF banner/buff, W wearable/passive, THR
throwable, CON consumable (lunchbox/drink), SHD shield/charge, BLD
building, CLK cloak/disguise. **Eff**: S under half a day, M 1-2 days, L
3-5, XL a week or more (all incl. data, hooks, one scenario test, not
sounds/models/viewmodel sequences, which each new class of weapon adds
roughly half a day for). **B**: batch. **Fit**: party gun game, Y/N/?
(see 5).

### Scout

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Scattergun | yes | S | HS | | | | Y |
| Force-a-Nature | no | N | HS | knockback on hit (self and victim), 2-clip | M | 2 | Y |
| Shortstop (primary) | no | N | HS | alt-fire shove, 4-pellet "pistol" shot pattern | M | 2 | Y |
| Soda Popper | no | N | HS | Hype meter, mini-crit air jumps | L | 2 | Y |
| Baby Face's Blaster | no | N | HS | Boost meter (speed from damage, lost on hurt) | M | 2 | Y |
| Back Scatter | no | N | HS | mini-crit from behind (backstab-angle test) | S | 2 | Y |
| Pistol (also Engineer; Lugermorph/C.A.P.P.E.R skins) | yes | S | HS | | | | Y |
| Bonk! Atomic Punch | no | N | CON | invulnerable drink, slowdown after | M | 2 | ? |
| Crit-a-Cola | no | N | CON | mini-crit drink, marked after | M | 2 | ? |
| Mad Milk / Mutated Milk | no | N | THR | jar: milk status, heal on hit | M | 2 | ? |
| Winger | no | N | HS | +jump (attribute), stats | S | 1 | Y |
| Pretty Boy's Pocket Pistol | no | N | HS | stats, health-on-hit | S | 1 | Y |
| Flying Guillotine | no | N | THR | returning cleaver, bleed, mini-crit at range | L | 2 | Y |
| Bat (Batsaber skin) | yes | S | MEL | | | | Y |
| Sandman | no | N | MEL+PRJ | ball projectile, stun by distance | L | 2 | ? |
| Holy Mackerel / Unarmed Combat | no | N | MEL | stats, kill-feed icon | S | 1 | Y |
| Candy Cane | no | N | MEL | health pack on kill, blast-weak | M | 2 | ? |
| Boston Basher / Three-Rune Blade | no | N | MEL | bleed, self-bleed on miss | M | 2 | Y |
| Sun-on-a-Stick | no | N | MEL | burning crits, no crits | S | 2 | Y |
| Fan O'War | no | N | MEL | mark-for-death | M | 2 | ? |
| Atomizer | no | N | MEL | extra air jump while held, mini-crit | M | 2 | Y |
| Wrap Assassin | no | N | MEL+PRJ | ornament projectile, bleed | M | 2 | Y |

### Soldier

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Rocket Launcher | yes | S | PRJ | | | | Y |
| Direct Hit | no | N | PRJ | stats; mini-crit vs airborne (hook) | S | 1+ | Y |
| Black Box | no | N | PRJ | stats; health on damage dealt | S | 2 | Y |
| Rocket Jumper | no | N | PRJ | stats (no damage to enemies, max ammo) | S | 1 | ? |
| Liberty Launcher | no | N | PRJ | stats | S | 1 | Y |
| Cow Mangler 5000 | no | N | PRJ | no ammo, charged shot, disables buildings, ammo-free | L | 3 | Y |
| The Original | no | N | PRJ | stats, rocket from the muzzle centre | S | 1 | Y |
| Beggar's Bazooka | no | N | PRJ | load more than the clip, misfire spread | M | 2 | Y |
| Air Strike | no | N | PRJ | mini-rockets, clip refilled on kill, rocket-jump handling | M | 2 | Y |
| Shotgun (all classes) | yes | S | HS | | | | Y |
| Reserve Shooter (Soldier, Pyro) | no | N | HS | mini-crit airborne, fast switch | S | 2 | Y |
| Panic Attack (4 classes) | no | N | HS | stats; fire-rate scale with clip-pump | M | 2 | Y |
| Buff Banner / Battalion's Backup / Concheror | no | N | BUF | rage meter, buff cone/aura, mini-crit/resist/heal | XL | 3 | ? |
| Righteous Bison | no | N | PRJ | piercing energy bolt, no ammo | M | 2 | Y |
| B.A.S.E. Jumper (also Demo) | no | N | W | parachute, fall control | M | 3 | ? |
| The Gunboats (W) | no | N | W | self blast damage reduction | S | 2 | ? |
| The Mantreads (W) | no | N | W | stomp bonus, air control, blast reduction | M | 3 | ? |
| Shovel | yes | S | MEL | | | | Y |
| Equalizer | no | N | MEL | low-health damage, speed buff | M | 2 | Y |
| Market Gardener | no | N | MEL | crit while rocket-jumping, -dmg otherwise | S | 2 | Y |
| Disciplinary Action | no | N | MEL | reach, speed boost on hit to ally | M | 2 | Y |
| Escape Plan | no | N | MEL | speed by missing health, no healing | M | 2 | Y |
| Pain Train (also Demo) | no | N | MEL | stats, capture rate | S | 1 | Y |
| Half-Zatoichi (also Demo) | no | N | MEL | honourbound: heal on hit, must kill to sheathe | M | 2 | Y |

### Pyro

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Flame Thrower (Nostromo Napalmer skin) | yes | S | FT | | | | Y |
| Backburner | no | N | FT | back-attack crits, no airblast; code-coded | M | 3 | Y |
| Degreaser | no | N | FT | fast switch, airblast cost, extinguish | M | 3 | Y |
| Phlogistinator | no | N | FT | Mmmph meter and crit window | L | 3 | Y |
| Rainblower | no | N | FT | skin look (rainbow flame) | S | 1 | Y |
| Dragon's Fury | no | N | PRJ+FT | fireball projectile, pressure meter | L | 3 | Y |
| Airblast (all flamethrowers; gate for several) | no | P | FT | reflect, push, extinguish | L | 3 | Y |
| Flare Gun | no | N | PRJ | flare projectile, ignite, crit vs burning | M | 2 | Y |
| Detonator | no | N | PRJ | detonate in flight, self jump | M | 2 | Y |
| Scorch Shot | no | N | PRJ | flare knockback, crit vs burning | M | 2 | Y |
| Manmelter | no | N | PRJ | extinguish allies, revenge crits | L | 3 | ? |
| Gas Passer | no | N | THR | gas cloud, ignite on later damage | L | 3 | ? |
| Thermal Thruster | no | N | W | jetpack launch, slam | L | 3 | ? |
| Fire Axe | yes | S | MEL | | | | Y |
| Axtinguisher / Postal Pummeler / Lollichop | no | N | MEL | crit vs burning (variant: forced crits, no extinguish) | S | 2 | Y |
| Homewrecker / Maul / Neon Annihilator | no | N | MEL | stats; vs-buildings damage; wet minicrit | S | 1 | Y |
| Powerjack | no | N | MEL | health on kill, speed, extra damage taken | S | 2 | Y |
| Back Scratcher | no | N | MEL | stats, bigger packs | S | 1 | Y |
| Sharpened Volcano Fragment | no | N | MEL | afterburn on hit, -dmg | S | 1 | Y |
| Third Degree | no | N | MEL | damages linked Medic heal chain | L | 3 | N |
| Hot Hand (slap) | no | N | MEL | speed on kill | S | 2 | Y |

### Demoman

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Grenade Launcher | yes | S | PRJ | | | | Y |
| Loch-n-Load | no | N | PRJ | stats, no-bounce, contact damage | S | 1 | Y |
| Loose Cannon | no | N | PRJ | cannonball, double-donk, impact | L | 2 | Y |
| Iron Bomber | no | N | PRJ | stats, short fuse, low bounce | S | 1 | Y |
| Stickybomb Launcher | yes | S | STK | | | | Y |
| Scottish Resistance | no | N | STK | aim-detonate, more stickies | M | 2 | Y |
| Sticky Jumper | no | N | STK | no damage, 8 stickies | S | 1 | ? |
| Quickiebomb Launcher | no | N | STK | charge-scaled damage, faster arm | S | 1 | Y |
| Chargin' Targe / Splendid Screen / Tide Turner | no | N | SHD | charge (alt) with turn control, resists, bash crit | XL | 3 | ? |
| Bottle (Scottish Handshake skin) | yes | S | MEL | | | | Y |
| Eyelander / Horseless Headless Headtaker / Nessie's Nine Iron | no | N | MEL | head counter (speed, health), decap crits | M | 2 | Y |
| Scotsman's Skullcutter | no | N | MEL | stats, slower, speed penalty | S | 1 | Y |
| Claidheamohmor | no | N | MEL | stats, long reach, charge meter hit | S | 1 | Y |
| Persian Persuader | no | N | MEL | pickup ammo refills charge, no ammo | M | 3 | N |
| Ullapool Caber | no | N | MEL | explosive melee, breaks | M | 2 | Y |

### Heavy

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Minigun | yes | S | HS | | | | Y |
| Natascha | no | N | HS | slow on hit, -damage | S | 2 | Y |
| Brass Beast | no | N | HS | slow spin-up, resist when spun | S | 1 | Y |
| Tomislav | no | N | HS | fast spin-up, silent | S | 1 | Y |
| Huo-Long Heater | no | N | HS | ring-of-fire burn around, uses ammo | M | 2 | Y |
| Iron Curtain, Deflector (MvM only) | no | N | HS | robot-only attributes | - | - | N |
| Shotgun | yes | S | HS | (shared) | | | Y |
| Family Business | no | N | HS | stats, clip | S | 1 | Y |
| Sandvich / Robo-Sandvich | no | N | CON | consume to heal, drop pack to team | M | 2 | ? |
| Dalokohs Bar | no | N | CON | temporary max health | S | 2 | ? |
| Buffalo Steak Sandvich | no | N | CON | mini-crit and speed, can't switch | S | 2 | ? |
| Fishcake / Second Banana | no | N | CON | health variants | S | 2 | ? |
| Fists | yes | S | MEL | | | | Y |
| Killing Gloves of Boxing | no | N | MEL | crit on kill, damage | S | 2 | Y |
| Gloves of Running Urgently | no | N | MEL | speed, health drain | S | 2 | Y |
| Warrior's Spirit | no | N | MEL | health on hit, damage | S | 1 | Y |
| Fists of Steel | no | N | MEL | resist vs ranged, -vs melee | S | 1 | Y |
| Eviction Notice | no | N | MEL | speed and health on hit | S | 2 | Y |
| Apoco-Fists | no | N | MEL | stats, extra reach, taunt | S | 1 | Y |
| Holiday Punch | no | N | MEL | laughter on crit, forced crit | M | 2 | ? |
| Bread Bite | no | N | MEL | stats | S | 1 | Y |

### Engineer

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Shotgun (primary slot) | yes | S | HS | (shared) | | | Y |
| Frontier Justice | no | N | HS | revenge crits tied to sentry | L | 3 | N |
| Widowmaker | no | N | HS | metal as ammo, damage | M | 2 | Y |
| Pomson 6000 | no | N | PRJ | energy beam, drains Uber/cloak | L | 3 | N |
| Rescue Ranger | no | N | PRJ | pull building, repair at range | L | 3 | N |
| Pistol (Scout's) | yes | S | HS | (shared) | | | Y |
| Wrangler / Giger Counter | no | N | BLD | sentry control, shield | L | 3 | N |
| Short Circuit | no | N | PRJ | energy ball, destroy projectiles | L | 3 | ? |
| Wrench | yes | S | MEL | | | | Y |
| Gunslinger | no | N | MEL | mini-sentry, third-hit crit | M | 3 | N |
| Southern Hospitality | no | N | MEL | bleed, crits vs burning, building speed | S | 2 | Y |
| Jag | no | N | MEL | stats, build speed | S | 1 | Y |
| Eureka Effect | no | N | MEL | teleport home, cheaper builds | M | 3 | N |
| Build / Destroy PDAs, toolbox (stock) | no | P | BLD | sentry, dispenser, tele, metal, upgrade | XL | 3 | N |

### Medic

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Syringe Gun | yes | S | PRJ | | | | Y |
| Blutsauger | no | N | PRJ | health on hit, no passive regen | S | 2 | Y |
| Crusader's Crossbow | no | N | PRJ | arrow, heals allies, bolt drop | M | 2 | Y |
| Overdose | no | N | PRJ | speed from Uber meter, slower | S | 2 | ? |
| Medigun | no | P | MED | beam heal, overheal, Uber meter | XL | 3 | ? |
| Kritzkrieg | no | N | MED | crit Uber | L | 3 | ? |
| Quick-Fix | no | N | MED | fast heal, speed matching, mega-heal | L | 3 | ? |
| Vaccinator | no | N | MED | resist types, small ubers | XL | 3 | ? |
| Bonesaw | yes | S | MEL | | | | Y |
| Ubersaw | no | N | MEL | Uber on hit | M | 3 | ? |
| Vita-Saw | no | N | MEL | organs keep Uber on death | M | 3 | ? |
| Amputator | no | N | MEL | taunt heal | M | 3 | ? |
| Solemn Vow | no | N | MEL | see enemy health | S | 2 | Y |

### Sniper

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Sniper Rifle | yes | S | HS | | | | Y |
| Huntsman / Fortified Compound | no | N | PRJ | bow draw, arrow arc and drop, headshot, arrows stick | L | 2 | Y |
| Sydney Sleeper | no | N | HS | jarate on charged shot, no headshot crit | M | 2 | Y |
| Bazaar Bargain | no | N | HS | head counter raises charge | M | 2 | Y |
| Machina / Shooting Star | no | N | HS | penetrate on full charge, tracer | M | 2 | Y |
| Hitman's Heatmaker | no | N | HS | Focus meter, headshot bonus | M | 3 | Y |
| AWPer Hand | no | N | HS | slower fire, damage, ammo | S | 1 | Y |
| The Classic | no | N | HS | charge without zoom, headshot on release | M | 2 | Y |
| SMG | yes | S | HS | | | | Y |
| Jarate / Self-Aware Beauty Mark | no | N | THR | jar, mini-crit, extinguish | M | 2 | ? |
| Cleaner's Carbine | no | N | HS | crikey meter, crit on full | M | 2 | Y |
| Razorback (W) | no | N | W | backstab block, shatter | M | 3 | ? |
| Darwin's Danger Shield (W) | no | N | W | stats, health | S | 1 | ? |
| Cozy Camper (W) | no | N | W | flinch-resist when scoped | S | 1 | ? |
| Kukri | yes | S | MEL | | | | Y |
| Tribalman's Shiv | no | N | MEL | bleed, -damage | M | 2 | Y |
| Bushwacka | no | N | MEL | always crit, take more fire | S | 1 | Y |
| Shahanshah | no | N | MEL | damage by health | S | 2 | Y |

### Spy

| Weapon (variants) | In | Spec | Family | New mechanics | Eff | B | Fit |
|---|---|---|---|---|---|---|---|
| Revolver | yes | S | HS | | | | Y |
| Ambassador | no | N | HS | headshot crits, spread | S | 2 | Y |
| L'Etranger | no | N | HS | cloak meter on hit | M | 3 | ? |
| Enforcer | no | N | HS | disguise bonus, piercing | S | 2 | ? |
| Diamondback | no | N | HS | crits per backstab | S | 2 | Y |
| Knife | yes | S | MEL | | | | Y |
| Your Eternal Reward / Wanga Prick | no | N | MEL | disguise as victim, silent kill | L | 3 | ? |
| Conniver's Kunai | no | N | MEL | health on backstab | S | 2 | Y |
| Big Earner | no | N | MEL | speed boost, cloak meter on kill | M | 3 | ? |
| Black Rose / Sharp Dresser | no | N | MEL | stats (skin of Knife; disguise bonus) | S | 1 | Y |
| Spy-cicle | no | N | MEL | fire immunity, ice kill | M | 2 | Y |
| Sapper (Red-Tape Recorder, Ap-Sap, Snack Attack) | no | N | BLD | sap, alt mechanics | L | 3 | N |
| Invis Watch (Quackenbirdt skin) | no | P | CLK | cloak, decloak, meter, fade | XL | 3 | ? |
| Dead Ringer | no | N | CLK | fake death | XL | 3 | ? |
| Cloak and Dagger | no | N | CLK | still-cloak regen | L | 3 | ? |
| Disguise Kit (PDA) | no | N | CLK | disguise tables, team models | XL | 3 | ? |

### All-class and event items (not planned)

Spellbooks (Halloween), Grappling Hook, Passtime Jack gun, Saxxy melee
(skin of class melee, see 1). Mark out of scope.

## 4. Phases

### Batch 0: the attribute layer (prerequisite) -- built 2026-11-23

User decision: items_game is **not** read at game run time. The
converter `tf2:item_stats` (`games/tf2/src/game/items/convert.rs`,
version `items::VERSION`) runs host-side in `chimera_pack sets tf2
--only items` (also in a whole `sets tf2`): it reads the install's
`scripts/items/items_game.txt` and each item class's `.ctx` script
(decrypted with the host's key, never printed), resolves prefabs and
folds each item's attributes by their `description_format` (multiply,
add, OR; integers rounded once at the end), and writes one record per
weapon item (class, slot, classes, models, folded attributes, the
script's base numbers, effective numbers, and from version 2 its looks:
`visuals`) into `chimera/gen/tf2.items/v<VERSION>/items.toml`, set `items` of the TF2 build's
manifest. Nothing of the schema is committed.

In the game (`games/tf2/src/game/items/`): data/items.toml (ours) names
the items we play by definition index and maps each item class to one
of our weapons (`[bases]`); `GunNumbers::with_pack` builds each item's
weapon document over its base (`items::docs`: damage, clip, reserve as
its own ammo type, interval, spread, projectile speed, blast radius vs
others with the owner's own radius and damage kept, fuse on 14-tick
checks, no-bounce and shatter, sticky arm/charge/charge damage/limit,
spin-up and spun speed, sword range and deploy, own-blast reduction and
jumpers' no self damage) and registers it with the stock ones (server
documents, so clients get them). The holder's numbers ride on the
`tf2:item` hook (`items::mods`): speed provided or while active, jump
while active (`SourceTraits::jump_scale`), damage taken by kind (and
the spun Heavy's resist), heal on hit and kill, ignite on hit, the
Bread Bite's max-health drain. Ladder eligibility: a weapon that deals
damage keeps its base's `class` (ladders draw it); data/items.toml's
`gun_game` overrides (utility items such as Jarate will set it false).

### Batch 1: existing mechanics, stats only (31 entries) -- built 2026-11-23

Liberty Launcher, The Original, Rocket Jumper, Direct Hit (stats part),
Loch-n-Load, Iron Bomber, Sticky Jumper, Quickiebomb, Brass Beast,
Tomislav, Family Business, Winger, Pretty Boy's Pocket Pistol, AWPer Hand,
Rainblower (skin look), Holy Mackerel, Pain Train, Homewrecker/Maul, Neon
Annihilator, Back Scratcher, Volcano Fragment, Scotsman's Skullcutter,
Claidheamohmor, Warrior's Spirit, Fists of Steel, Apoco-Fists, Bread Bite,
Jag, Bushwacka, Black Rose, Darwin's, Cozy Camper.

Status (scenario tests: games/tf2/tests/it/items.rs; the converted
install: tests/it/heavy/items.rs):

- **Complete in numbers** (every attribute that has an effect in a party
  match applied): Winger, Pretty Boy's, Liberty Launcher, Rocket Jumper,
  Family Business, Brass Beast, Warrior's Spirit, Bread Bite, Iron Bomber,
  Sticky Jumper (bar its model), Scotsman's Skullcutter, Volcano Fragment,
  Back Scratcher (packs/healers recorded, none in a party match),
  Homewrecker, Maul, Fists of Steel (ranged/melee taken, the next
  deploy's ×2), Holy Mackerel, AWPer Hand, Black Rose.
- **Completed by batch 2**: Direct Hit (mini-crit vs a blast-launched
  victim), Bushwacka (mini-crits become crits).
- **Partial**: Neon Annihilator (crit vs wet: batch 2), Quickiebomb (destroying enemy
  stickies), Loch-n-Load (shatter and stats done; vs buildings), Tomislav
  (silent spin sounds), The Original (centre fire done; its models),
  Claidheamohmor (stats, sword and damage taken; the shield charge is
  batch 3), Pain Train (bullet vulnerability; capture value n/a), Jag
  (building numbers n/a), Apoco-Fists (gib on crit kill: crits off),
  Rainblower (the rainbow look), Darwin's and Cozy Camper (converted and
  mapped, but nothing wears a wearable yet; regen, afterburn immunity,
  scoped rules not applied).
- **For every item** (2026-11-23, converter version 2: the record's
  `visuals`): its own `c_` model on its class's arms (view and held), its
  skin, kill icon (`item_iconname`, else its base's) and the sound
  entries its `visuals` name (shot, Minigun crit shot, empty, melee
  hit/miss); its view-model sequences, stance and effects are its base's
  (`items::stock_of`); the holster multiplier and the sword's ×1.75 from
  the switched-from side apply (`items::mods::holster`). Left: `anim_slot`
  and `animation_replacement`, its own muzzle/tracer/attached particles
  (recorded only).
- **Per-item asset sets** (2026-11-23): each item weapon's
  `weapon/<name>` (`weapon/direct_hit`; `tf2::game::sets::item_pieces`,
  `chimera_pack sets tf2 --only weapon/ --pack`): its own model, skin's
  materials, held model and the waves of its own sound entries, less
  `base`, `common`, `players`, its classes' `players/<class>` (the arms
  and their sequences) and its base's `weapon/<id>`. A map's dressing
  loads only the items whose models are here (`items::on_mount_here`),
  so no load fetches every item's set; an item whose set comes later
  (the sandbox's pick) is built then (`tf2::game::late_items`: view and
  held models, sound entries). Published: 31 sets,
  0.6-6.3 MB stored each (2.4 MB median), 392 records, 65 MB (60.6 MB of
  it in no other set). A match's offer and the sandbox's unit fetch name
  it as any gun's, so a player served by the asset service with no
  install sees the item's own model (heavy test
  `heavy::items::an_items_set_serves_its_own_view_model_from_the_store_alone`).

### Batch 2: small new mechanics (68 entries)

Each mechanic is reused by several weapons; build it once.

**Built 2026-11-23** (specs/tf2/batch2_mechanics.md; `game::items::effects`,
`game::weapons::flare`, Chimera's `status`: a bleeding wound and a speed
boost any game can give; scenario tests: games/tf2/tests/it/batch2.rs):
each mechanic a rule named by the item's attributes on its `tf2:item`
hook, each weapon data/items.toml's entry plus those.

- *Health on hit/kill*: Black Box (per rocket, by enemy damage), Blutsauger
  (3 a needle; its -2 on the Medic's regeneration, now built: 3 rising to
  6), Powerjack, Conniver's Kunai (backstab heal to 3x, overheal drain),
  Candy Cane (a small kit dropped, taken for 20 %, 32 s), Eyelander,
  Headtaker and Nessie's Nine Iron (heads: health and speed up to 4).
- *Mini-crit/crit conditions*: a victim in the air after another's blast
  (`BlastLaunched`: the Direct Hit now complete, the Reserve Shooter),
  the Market Gardener, the Back Scatter, burning victims (Flare Gun crit,
  Detonator and Scorch Shot mini-crit), the Axtinguisher and Postal
  Pummeler (fitted burn law, burn put out, boost on a burning kill); the
  Escape Plan's mark; the Bushwacka's mini-crits as crits (now complete).
- *Bleed*: Boston Basher and Three-Rune Blade (with the miss's self hit),
  Tribalman's Shiv, Southern Hospitality. Flying Guillotine and Wrap
  Assassin wait for their projectiles' spec.
- *Flares*: Flare Gun, Detonator (attack2 detonation, own blast), Scorch
  Shot (bounce, push, floor explosion) on an abstract flare gun base
  (data/weapons/flaregun.toml).
- Their asset sets: the 26 new `weapon/<name>` sets published
  2026-11-23 (`chimera_pack sets tf2 --only weapon/ --pack --sign`: 57
  item sets now, the Candy Cane's with the small kit's model).
- *Speed buffs*: the speed boost (x1.4, +105 at most), Escape Plan,
  Equalizer, Disciplinary Action, Eviction Notice, Gloves of Running
  Urgently, Atomizer (an air jump while out).
- Provisional numbers (the spec's open questions) are each module's Known
  shortcuts: the Detonator's own blast law and push, the kit's life and
  reach, the head excess's drain, heads lost at death, the Axtinguisher's
  law beyond flare burns.

Still to build in batch 2:

- *Mini-crit conditions*: wet (Neon Annihilator's crit).
- *Bleed*: Wrap Assassin, Flying Guillotine (their projectiles first).
- *Knockback on hit* (Force-a-Nature), *shove* (Shortstop).
- *New projectile types*: arrow (Huntsman, Crossbow), ball (Sandman),
  cleaver/ornament, energy bolt (Bison), cannonball (Loose Cannon),
  explosive melee (Caber).
- *Consumables*: lunchbox (Sandvich family), drinks (Bonk, Crit-a-Cola),
  jars (Jarate, Mad Milk): one "use and cooldown" slot plus a status.
- *Meters*: head counter (Bazaar; the Eyelander's is built), Hype (Soda Popper), Boost
  (Baby Face's), crikey (Cleaner's Carbine), kill-based refills.
- *Sniper variants*: pierce, Classic's charge, Sydney's jarate shot.
- *Revolver variants*: Ambassador, Diamondback, Enforcer.

Effort: ~1-2 days per mechanic family, then weapons are batch-1 cheap.
Needs a spec for every named number the attribute list does not give
(durations, meters, drop of arrows, bleed ticks, knockback force).

### Batch 3: big mechanics (40 entries)

| Group | Weapons | Fit in a party gun game | Spec needed |
|---|---|---|---|
| Medigun and Uber | Medigun, Kritzkrieg, Quick-Fix, Vaccinator, Ubersaw, Vita-Saw, Amputator | Only with team modes: needs a teammate; dead weight in free-for-all and 1v1. Offer Medigun as a team-mode rung only after 3 shipped. | Yes, large |
| Banners | Buff Banner, Battalion's Backup, Concheror | Team modes only; no use solo | Yes |
| Shields and charge | Chargin' Targe, Splendid Screen, Tide Turner, Eyelander charge | Y in gun game with melee close-quarters: charge works solo; make a rung | Yes |
| Cloak, disguise, Sapper | Invis Watch, Dead Ringer, Cloak and Dagger, Disguise Kit, Sapper | Disguise needs teams of players and Engineer buildings; cloak is fun solo. Defer; no Sapper | Yes, large |
| Building PDA and Engineer kit | Build/Destroy PDAs, Gunslinger, Wrangler, Frontier Justice, Rescue Ranger, Pomson, Eureka Effect, Third Degree | The existing backlog item "an Engineer rung" covers the stock sentry/dispenser/tele only; extras after | Yes |
| Airblast and fire mechanics | Airblast, Phlogistinator, Backburner, Degreaser, Dragon's Fury, Manmelter, Gas Passer | Y, strong in gun game: airblast first, others reuse it | Yes (partly specified: reference only) |
| Movement kit | B.A.S.E. Jumper, Mantreads, Thermal Thruster | Fun but needs air control spec | Yes |
| Misc | Cow Mangler (charged shot, Y), Hitman's Heatmaker (Y), Razorback (backstab block, with Spy only) | | Yes |

## 5. Fit with "put guns into existing modes"

Guns (HS, PRJ, FT, STK, STK, THR and melee variants) drop into the existing
Ultimate Gun Game rungs as new offers; the TF2 rung lists stay data. No new
minigame. Weapons whose point is a team (Medigun, banners, Engineer kit,
Sapper, disguise) are not rung weapons: they wait for a team mode that
asks for them. "?" in the tables is exactly these and consumables whose
rung behaviour (consume, then what?) needs the user's call.

## 6. Which need a spec (clean-room measurement) first

Specs are written in a separate session from real TF2 behaviour, not from
code. Order by what batch 1/2 needs:

1. Attribute semantics note (what each attribute number does; one test per
   attribute in a controlled map). Covers batch 1 entirely.
2. Health-on-hit, mini-crit conditions, bleed, speed buffs, knockback
   numbers, flare/arrow/ball/cleaver/bolt/cannonball projectile numbers,
   lunchbox/drink timings, meters (batch 2). **Specified 2026-11-23** in
   [specs/tf2/batch2_mechanics.md](../../../specs/tf2/batch2_mechanics.md)
   (measured): health on hit/kill, mini-crit conditions, bleed, the flare
   projectile (Flare Gun, Detonator, Scorch Shot) and the melee speed
   buffs. Still to spec: knockback on hit of the Force-a-Nature and the
   Shortstop's shove, the arrow, ball, cleaver/ornament, bolt and
   cannonball projectiles, lunchbox/drink timings, meters, sniper and
   revolver variants.
3. Airblast, shield charge, cloak/disguise, medigun, banners, building
   rules (batch 3): each its own spec file, the large ones in parts.

## 7. Recommended order

1. **Batch 0 + 1** together: the attribute layer proves itself on the ~25
   stat-only weapons, with the spec in item 1 above. Everything after is
   "data file + hook".
2. **Batch 2 families that cover the most weapons per mechanic**: health on
   hit/kill, mini-crit conditions, bleed, flare projectile, melee speed
   buffs (about 25 weapons together).
3. Unlocks with the best value per effort for the party: Eyelander (head
   counter), Huntsman, Flare Gun/Detonator, Sandvich, Chargin' Targe,
   Pomson skipped.
4. Batch 3 groups in the order airblast, shields, then movement; medigun
   and building waits for team modes.
