# Onboarding and sign-in

A new player's first minutes: from an invite to the main menu, signed
in. This plan covers the design only, and no code is written until it
is settled and mocked up. It replaces the first-run page of
[hub.md](hub.md) S3 and changes some of that plan's account decisions
(below). Backlog: "Onboarding and sign-in flow, with motion design".

## Principles

- **The player's view first.** A player needs two things, the hub and
  a credential, and gets both from one paste: the invite link carries
  the hub's address, its certificate fingerprint and a one-time code.
- **No passwords.** People don't want to invent a password for one more
  site, or wonder whether this one will leak. An account is held by a
  device key made on the player's PC, and can be got back at will: a
  recovery key, a link code from another signed-in device, or a fresh
  invite from the hub owner.
- **Invite only.** Only the hub owner makes invites; there is no
  member-made "invite a friend". Open registration is a large risk and
  stays out (Later).

## Decisions (user, 2026-11-17)

1. **Invites are codes with an intent, not names.** An invite is a
   one-time redemption code. The owner records why it was made (an
   intent such as "for Sam, sent on Discord") and may name an intended
   recipient, so an unwanted redemption can be traced and revoked. The
   invite carries no username.
2. **Players choose their username** at sign-up, and can change it at
   will later in game, as on Steam. This replaces hub.md decision 11's
   permanent handles. The account's id is what stays fixed. Renames are
   limited to one a minute, and the last 10 names are kept as a visible
   history against impersonation (low priority).
3. **One field takes any form.** The redeem field accepts the full
   link, a URL containing the code, or the bare code, parsed when the
   player presses Enter (by pattern). A link already on the clipboard
   is filled in automatically when it matches.
4. **Clicking an invite link may open the game** with the field filled
   in. This needs the link handler registered on Windows (and Linux).
5. **Playing without an account is a small skip link**, not a main
   choice: LAN, direct connect and own installs as today, without
   parties, the hub's list or downloads.
6. **Where sign-up happens:**
   - **In the client:** paste the code, choose a name, save the
     recovery key. The device key is made right there, so the client
     is signed in when it ends. This is the shortest path.
   - **A web page served by the hub,** in the same design language:
     redeem, choose a name, save the recovery key. It ends with a link
     code or an "open the game" link that signs the client in, because
     the device key must still be made on the player's PC.

   Decided (user, 2026-11-17): the client is the primary path; the web
   page is optional and later, for players who haven't installed the
   game yet.

## Decisions (user, 2026-11-21: the mockups approved)

The user reviewed the "Onboarding Mockups" (nine screens: the title
moment, paste your invite, redeem and its errors, confirm the hub,
choose a name, the recovery key, done, "I already have an account",
the returning player) and said to start building, with these changes:

7. **A first-launch intro**: on the very first launch only, a
   motion-graphics intro with the logo and a creative welcome
   ("Welcome to Lucker Party"-style). Built last.
8. **No blocking animations, on every screen**: things animate in,
   but the text and the call to action are usable at once, and
   pressing it skips what is still animating.
9. **Paste your invite** as drawn, with "Play without an account" more
   prominent (still the tertiary choice), and the live "what an invite
   carries" pane kept.
10. **Choose a name** also picks an **avatar and colour** from the
    existing ones (`cl_avatar`'s 32 looks: glyph x 8 + colour), kept on
    the account. A taken name answers with a few free suggestions (a
    simple algorithm).
11. **Redeem and its errors** (wrong or used code, hub unreachable,
    certificate mismatch), **confirm the hub** with its MOTD, the name
    rules, the **recovery key** and screens 8-9 as drawn.
12. **The main menu keeps servers behind Play**: no server list on the
    main menu (the Done screen's list in the mockup is dropped). The
    base assets everything needs download up front; a hub's own
    customisation later. Future note: an option to download bulk asset
    sets.

## Decision (user, 2026-11-21: start fresh)

13. **Accounts are the only way in.** Everyone starts fresh with an
    invite and this first run, testers included: there are no other
    credentials for the hub and nothing carried over (hub.md, section 6).
    The first run shows whenever this PC has no account and no skip mark.
    A plain token stays only for a game server's own asset service with
    no hub (`chimera_pack token add`, `+chimera:asset_token`).

## Decisions (user, 2026-11-21: the first run tried)

The user went through the built first run and asked for these; each is
built (`party_kit::onboarding`, `account_page`, the kit):

14. **One ground**: the first run stands on the main menu's own ground
    (the kit's `shell::menu_ground`, the ambient Settings uses too), not
    a look of its own; the account page uses it as well.
15. **The invite checks itself**: no "press Enter to check". The field is
    checked with its hub at once on a paste, else 0.6 s after the last
    key (`INVITE_PAUSE`), staying on Welcome; Continue then goes on with
    no other call (pressed while the check runs, it waits on Checking).
    When the window comes back to the front, an invite link on the
    clipboard fills the field and is checked, if the field is empty or
    holds something else that isn't a checked invite: a checked one (or
    one being checked) is never replaced. The clipboard's link at the
    start is checked the same way.
16. **No entrance slide**, as a kit rule (`chimera_ui::shell`'s doc):
    screens appear in place, whole; the intro and the controls' own
    small motions stay.
17. **Addresses hidden by default, everywhere** (streamers): the kit's
    `chimera_ui::secret` shows a masked value with an eye key (like a
    password field), masked again whenever a screen opens. The first
    run's field, pane, Checking and Hub; the account page's hub; Join's
    Address field, Recent and servers named by their address (one eye
    for the page); the party's "your party is on" and the Host page's
    UPnP line mask theirs with no eye. Copying never needs the reveal.
18. **The avatar with what you change**: on Choose a name, the 32 looks
    sit at the left beside the fields, in tiles as big as the room
    allows; the right-hand pane shows only the preview (what follows from
    your choices). The account page draws the grid under the avatar's
    row, across the list; its help tile keeps only words.
19. **Forms look like forms**: the kit's `form::form_field` (a title, a
    line on what the field is for, the input in the smaller typed size)
    for the username and display name here and the account page's
    username. Kept minimal and in one place: a forms design pass
    restyles it (and the Invites tab) later.
20. **The recovery key hidden by default** with the same eye key; Copy
    and Save as file work hidden.
21. **Save as file says where**: the whole path of the saved key file,
    with Copy path.

## The flow (client)

1. **First launch:** a short title moment with motion (the kit's
   `chimera_ui::motion`, slowed or off by the motion setting). Then one
   screen with the redeem field as the hero ("Paste your invite"), a
   secondary "I already have an account", and a small "Play without an
   account" link.
2. **Redeem:** the one field, checked as it is pasted or typed
   (decision 15). While checking it says "Reaching the
   hub…", and a wrong or used code, an unreachable hub, or a bad
   fingerprint each say what happened and what to do.
3. **Confirm the hub:** "You're joining *<hub name>*", with its message
   of the day. This builds trust and catches a wrong link.
4. **Choose a name:** the username, checked live against the rules
   and for availability, and the display name, which defaults to it.
   Both can change later.
5. **Recovery key:** in plain words ("gets your account back if you
   lose this PC"), with Copy and Save as file, and "I saved it" before
   continuing.
6. **Welcome:** the main menu with the server list filled. Downloads
   start in the background, shown by the same progress display as
   loading (the one load state; lobby-between-maps.md step 5).

**Returning players** see no sign-in screen: they are signed in
automatically, with an account chip in a corner. If the hub can't be
reached, a banner says so and offline play goes on.

**"I already have an account":**
- **Link a device:** a short code shown in Settings on the signed-in
  PC, typed on the new one, like pairing a TV app.
- **Recover:** paste the recovery key.

## Conformance pass 2026-11-21

The user asked for a pass over the built screens against the approved
"Onboarding Mockups", the UGG style references (Logo Set v0.4, Look
Book V, Display Proofs, `shell.toml` and the UGG theme) and the newer
kit rules (ui-kit review of 2026-11-21), and pointed out that the
screens carried a simplified stand-in, not the game's logo. Screenshots
of all 17 staged screens, before and after, at 1920x1080 (offscreen).

**The logo.** The bar drew three slanted bars and "LUCKER PARTY" in
Display Black, the mockups' placeholder. The game's logo is the UGG
lockup: the `mark` (three helmeted profiles, `chimera/ui/data/
icons.toml`) beside ULTIMATE spread over GUN GAME in the wordmark role
(UGG Display Black, ss02), the same one the main menu
(`signal_menu`) and the Esc menu (`Ui::lockup`) draw. The bar now draws
that lockup (`Ui::lockup_drawn`: its mark from the kit's icon set, no
image assets needed) at the lobby header's size (1.05 cq), the screen's
place after a hairline in Mono. `Ui::lockup` now draws the same way.

**Differences found, screen by screen** (F: fixed in this pass; L: left,
with the reason):

- All screens:
  - F: the logo (above).
  - F: the ground was flat ink with an 8 % cobalt wash; the mockups'
    ambience is a navy fall from the top left (a gradient of the
    theme's hero tone). L: its two radial glows band visibly in Bevy's
    8-bit UI gradients at such low alphas, so they are left out.
  - F: the places were numbers in the code; now `[layout.onboarding]`
    in the theme (margin, bar, rule, lockup, column, pane, corners).
  - F: fields were the settings row's (1.9 cq, typed text .74); the
    mockups' field is the screen's hero (taller, bigger type): the
    kit's `Ui::text_field_hero` (`[layout.field] hero_h` 3.2, `hero_text`
    .9), used by every first-run field.
  - F: the explaining panes lacked the blueprint corner brackets
    (`Ui::corners`).
  - L: the mockups' faint blueprint grid on the right half: not drawn
    (dozens of lines a screen for little); the gradients carry the
    depth.
  - Kept as built (the newer kit rules win over the mockups): tabs grey
    to white with no underline (the merged kit's `tabs_badged`).
- Welcome, clipboard: F: the pane's link diagram (the link's shape with
  its three parts underlined and numbered, the parsed values once read)
  was missing. F: a long link now shows its fingerprint as its first and
  last four, so it fits the hero field.
- Checking: F: the three checks had no lights (now `Ui::kv_lit`: under
  way in accent, waiting unlit; their words in the data).
- Used, unknown: F: the buttons were Continue and "I already have an
  account" on every error; the mockups' hero is the one action that
  helps: "Try another" (Enter clears the field) with "I already have an
  account".
- Unreachable: F: hero Retry with "Play without an account" as the
  ghost.
- Certificate: F: the field stayed (editable) and Continue led on; now
  no field, hero Back (Esc) and the ghost "Copy details for the owner"
  (the host and both fingerprints to the clipboard), Enter does nothing;
  the fingerprints lit (the invite's accent, the hub's threat), both in
  groups of four.
- Hub: F: the hub's emblem (its initials on the hero slant) and the note
  beside it; the rows' lights (the certificate's in accent when pinned).
  L: the mockups' "Servers" row: the check answer carries no server
  count yet.
- Name, name-bad, name-taken: F: the free names were outlined Mono
  boxes; now the kit's small secondary buttons. F: the preview lacked
  the kill-feed line (your kill outlined and barred in accent, the kill
  icon, the victim). The avatar picker (decision 10) stays in the
  preview pane; one marker a cell (the selected one outlined).
- Key: F: the key box's label sits on its top edge (a legend), as drawn.
  F: "I saved it" was a square box with a text tick; now the kit's check
  box on the slant (`Ui::check`, the tick an icon: `tick`).
- Have, recover: F: the link code was a plain field; now six code cells
  with a dash after the third, the caret in the next empty one (the
  plain field returns for anything longer). F: the inset "On your other
  PC" plate at the bottom right (where the code comes from). F: the bar's
  "‹ Back" showed a missing glyph (the Mono face has no ‹): now the
  kit's `back` icon.
- Done: F: the toast was a rectangle; now the hero slab slanted at both
  ends, as drawn.
- Returning, offline: F: the banner lacked the warning tint and bar at
  its left; its words were in the code, now `[chip]` in
  `data/onboarding.toml` with the chip's lines. L: the chip sits under
  the shell's rack, not inside it (the rack is the shell's); the
  banner's "Retry in 0:24" countdown isn't drawn (it would rebuild the
  chip every second).
- Motion: entries slide in, never blocking (decision 8), unchanged.

Tests: `party_kit` `onboarding::each_error_offers_its_one_action`,
`the_bar_carries_the_games_lockup`.

## The account page (2026-11-21)

Settings > Account (`party_kit::account_page`), opened by a click on the
main menu's account chip. Decisions made building it:

- **A shell screen of its own, "Main menu ‹ Account"** (it opens from the
  chip and goes back to the menu; the bar's Profile tile white), not a tab inside
  Settings' window: Settings' rows are cvars and binds with reset keys
  and pending changes, the account's are calls to the hub. It keeps
  Settings' look and the kit rules of 2026-11-21: tabs grey to white
  with badges (Invites counts the open ones), one row marker, every
  control in the one control column, the focused row's help in the tile
  at the right, the action bar saying what the last call did.
- **Profile**: username (Enter edits; availability asked of the hub
  0.45 s after the last key through a new ticket call, `POST
  /accounts/username/check`, which answers the sign-up's free names when
  taken; Enter renames, one a minute; the chip and `account.toml`
  follow), avatar (arrows or the 32 looks in the help tile: `cl_avatar`
  at once, the hub told 0.6 s after the last change), the hub (name,
  address, online light), Link another device (the 10-minute code in
  cells, `POST /accounts/link` as designed in hub.md 5.2), Sign out.
- **Sign out asks in its own row** (Stay with Esc, Sign out), no modal
  and nothing animated. It revokes this device at the hub (new `POST
  /accounts/sign-out`, hub.md 5.2's "Sign out of this device"), and
  deletes `account.toml` even when the hub can't be reached (the device
  then stays listed for the owner to revoke; the first run says so).
  The first run's Welcome follows at once, without the intro.
- **Invites for an owner in the game**: the Invites tab shows for an
  account whose role is `owner` (`/accounts/me`); the role is read again
  at each call, so a demotion counts at once. Intent (required),
  recipient (optional), role Tester or Host (segmented): an owner invite
  stays a command-line act. Make invite shows the link once on a plate
  with Copy (Ctrl+C); the link is the hub's own when it knows its public
  address, else made from the account's hub. The open invites (unused
  and live, newest first) each have Revoke, by the invite's row id (`POST
  /accounts/invites/revoke`; the CLI's revoke by name stays). New ticket
  calls: `POST /accounts/invites`, `GET /accounts/invites`. The admin web
  page's invite form takes the intent and recipient too.
- **Base files up front** (decision 12, `party_kit::base_download`): the
  first run's end starts the startup fetch (the startup sets of the
  app's games, `chimera_assets::sets::STARTUP_SETS`) that a start already
  signed in makes, its plan on a thread; a strip at the main menu's
  bottom left shows the loading screen's lines and bar and takes no
  clicks or keys. No restart (hub.md 8's "restarts once" is dropped): the
  games whose files arrived are read from the store at the next join
  (its bind remounts them) or start; a join meanwhile takes the fetch
  over on its loading screen.

Not yet here: display name, the device list with Revoke, a new recovery
key (hub.md 8).

## Form components (proposal 2026-11-21, for the user's review)

The user: the Account page's invite link should be an obvious button in a
form, not a settings row; make form components for the account and
onboarding pages. Mockup: "Form Kit Mockups" (claude.ai artifact, a
component sheet and five screens). Not built yet; `chimera_ui::form`
keeps the settings rows, these would sit beside them.

- **Form field** (`form_field`): a title (Display bold, 1.1×, uppercase,
  larger than the value at 1×), an optional Required/Optional tag, one
  help line always shown, the input box, and a status slot under it that
  is reserved so nothing moves. States: rest, focused (accent underline),
  checking (busy light, "Checking…"; asked 0.45 s after the last key),
  success (accent underline, a tick, e.g. "Available"), error (red
  underline and tint; says what is wrong and offers the fix as a click,
  e.g. free names), disabled (dimmed, a lock, why and until when).
- **Masked field** (`secret_field`): the value as dots with an eye key
  (pressed while shown) and a Copy key (a tick and "Copied" for 2 s).
  Copy works while hidden. For hub IPs, the recovery key, invite codes.
- **Buttons in a form**: one primary per form (the cobalt slab, cut
  corner; busy keeps its width and pulses its light, the form stays
  usable; disabled is ink), secondary (outline), tertiary (text, grey to
  white, no underline), destructive (red outline, filled only in its
  confirm step).
- **Result card** (`result_card`): what the hub hands out once (an invite
  link, a device code). Accent bar and tint, "Shown once", the whole
  value wrapped in mono, a primary Copy on the card (Ctrl+C; reads
  "Copied" on white after), a tertiary "Make another", and the facts
  (why, role, expiry). Invites: the form becomes this card after Make
  invite; it stays until the page is left.
- **Choice in a form**: the kit rule (segmented when every option fits
  an equal share, else a dropdown), as wide as the text fields.
- **Avatar field** (`avatar_grid`): all 32 looks at once, 8 × 4, no
  paging; the pick framed white with the accent bar under it; arrows
  move, Enter picks. On the left with the other editable fields.
- **Section header** (`form_section`): the slanted accent block, the
  title, a count, a hairline; one sentence under it.
- **Read-only row** (`value_row`): mono key, value, a lock (or the eye
  and Copy for secrets). Only these go in the right-hand tile: what the
  player can't change (hub, address masked, role, member since).
- **Path row** (`path_row`): a saved file's whole path, wrapped, never
  cut with an ellipsis, with "Show in folder".
- **Action bar** (`form_footer`): the last line of a form page; what the
  last call did on the left (changes pending, saving, saved at), actions
  on the right, primary last. Proposed: Profile's changes (username,
  display name, avatar) go out together with Save changes.
- **Badge** and **check**: the kit's one badge (outline, accent,
  warning) for roles and counts; the slanted check box.

Rules: one row marker (lighter background and the blue left bar) for
list rows such as open invites; nothing blocks input or slides in;
states change in place. Onboarding's paste field checks by itself after
a paste or the last key (no "press Enter to check"); Continue lights
when the invite is recognised. The recovery key's Continue waits for "I've
saved my key" or a save, never a timer.

## The owner's side

Invites stay a command for the hub owner, with the intent and an
optional intended recipient recorded: `chimera_accounts invite --intent
"<why>" [--for "<who>"]`, printing the link and the bare code. Each
redemption records the code's intent with the new account. Later, an
admin page lists invites by intent, with status and Revoke.

## Later

- Open registration, opt-in per hub, in rising risk: invite requests
  the owner approves; invites members can hand out within limits; open
  sign-up, which needs rate limits, abuse handling and likely email
  verification.
- The web sign-up page (decision 6).

## Next steps

**Now:**

1. Settings > Account's rest: the display name, the device list with
   Revoke, a new recovery key.
2. A hub's own sets after the base files; bulk sets an option (future).

**Done earlier:**

1. Mockup screens in the party's visual direction (futuristic
   broadcast-tech, the kit's controls), for the user's review: the first
   screen, redeem with each error, confirm the hub, choose a name, the
   recovery key, welcome, link a device, recover, and the returning
   player's chip.
2. Then the build, replacing hub.md S3's first-run page: account
   changes (invite intent, chosen and renamable usernames), the client
   flow, the link handler, and tests.

## Progress

- 2026-11-17: the design and the user's decisions.
- 2026-11-21: the mockups approved with decisions 7-12; the build
  started.
- Milestone (a), the hub's side, done: `chimera_assets::codes` (moved
  from the services, so the client checks the same rules live) with
  `choose_username` (a chosen name starts with a letter),
  `UsernameRules` (the checklist) and `suggestions` (the name with a
  digit, with `_` and a short word, then `_10`-`_99`); accounts
  migration 2 (`0002_onboarding.sql`: invites' `intent` and
  `recipient`, accounts' `invite_intent`, `avatar`, `renamed_at`,
  `username_history`); `chimera_accounts invite --intent "<why>"
  [--recipient "<who>"]` (prints the link and the bare code; the older
  `invite <name>` still names its player, and `--for <username>` stays
  the re-invite into an account, so the recipient is `--recipient`, not
  the plan's `--for`); an invite with an intent wants a chosen name at
  redeem (400 "choose a username"), the account keeps the intent and
  recipient; `POST /accounts/check` (the code's kind, the hub's name,
  MOTD and active players from hub.toml in the same store, a name's
  availability and 3 suggestions; nothing used up, wrong codes count
  toward the address's limit); `POST /accounts/username` (one a minute,
  the last 10 names in `previous_usernames`); `POST /accounts/avatar`
  and the avatar in redeem and `/accounts/me`; recovery by the key
  alone. Client side in `chimera_assets::account`: `InviteInput`,
  `Failure`, `check`, `redeem_with`, `rename`, `set_avatar`, `me`.
  Tests: `chimera_services` `accounts::an_invite_with_an_intent_wants_a_chosen_name`,
  `renames_keep_a_visible_history`, `a_recovery_key_alone_finds_its_account`,
  the certificate and unreachable failures in
  `a_hub_link_signs_in_through_pinned_tls`; `db::version_one_moves_to_onboardings_columns`.
  Not done: the admin page's invite form still takes a name only (the
  intent through the command line).
- Milestone (b), the client's screens, done: `party_kit::onboarding`
  (its words in `apps/party_kit/data/onboarding.toml`, the look the
  kit's: text fields, buttons, tabs, progress, kv rows, avatars) over the
  main menu at the app's start when this PC has no account and no skip
  mark (`first_run_due`): Welcome (the one field, the clipboard's link
  filled in, the live "what an invite carries" pane, "Play without an
  account" as a visible ghost button with its note), checking, the four
  errors (used, unknown or withdrawn, unreachable, another certificate
  with both fingerprints), the hub with its MOTD, the name (rules lit,
  availability 0.45 s after the last key with suggestions, the display
  name, the 32 avatars, the preview), the recovery key (Copy, Save as
  file into Documents, "I saved it"), "I already have an account" (link
  code or recovery key, the hub's link asked when none is known; a
  recovery shows the new key, unlike the mockup, since the old one stops
  working), and the main menu's "Welcome, <name>" toast. Entries slide
  in but never block (a key or click ends them; Enter works on the first
  frame). The display name is sent with the redeem (`RedeemRequest::
  display_name`). Tests: `party_kit` `onboarding::` (the whole path, the
  errors, the entry, a link code, recovery and Skip, every staged
  screen's words); `ui_scene onboarding <screen>` (OBSERVABILITY 2).
- Milestone (c), the returning player, done: no screen; the tickets
  come silently as before (`keep_fresh`, `use_user_hub`); the main
  menu's account chip under the rack (avatar, display name, `@username ·
  hub`, a green or amber light) and, while the hub's last call failed,
  the offline banner ("Can't reach <hub>", offline play goes on, Retry
  now, retried every 30 s). Test: `onboarding::a_returning_player_sees_
  the_chip_and_the_offline_banner`; `ui_scene onboarding
  returning|offline`.
- Milestone (d), the first-launch intro (decision 7), done 2026-11-21:
  on the first run's first start only (no account, no skip mark, no
  `onboarding-intro-seen` mark beside `account.toml`; the mark written
  when it ends or is skipped), never for returning players. 2.6 s at
  motion scale 1, over Welcome: the blueprint hairlines and a broadcast
  bar (channel and app, ON AIR, a running timecode) come up (0-0.4 s);
  the logo pass's bands sweep on the slant, the hero slab with the mark
  standing over the middle (0.05-0.9 s) while the game's own lockup comes
  up under it (`Ui::lockup_parts`, the same nodes as `lockup_drawn`: the
  mark pops, ULTIMATE tracks in, GUN GAME flickers on, 0.4-1.0 s); a rule
  scans out and "Welcome to <app>" (the app's own name) rises with a
  tagline (0.9-1.5 s); a system check types in (display, audio, games
  installed, account: none yet, 1.3-1.9 s); a tick bar runs the length;
  it fades into Welcome (2.3-2.6 s), whose entry plays under the fade.
  Never blocking: any key or click shows Welcome at once, settled, and
  the field types on the next key; `ui_motion_scale` stretches it, 0
  shows its last frame still for 0.6 s. A sting from the app's UI sounds
  (`pass_sweep`, `end_reveal`). Its words in `data/onboarding.toml
  [intro]`. No soft radial glows (the screens' linear navy fall only).
  `ui_scene onboarding intro [t]` (held at t). Tests: `onboarding::
  the_intro_plays_on_a_first_launch_only`, `any_key_or_click_skips_the_intro`,
  `the_intro_ends_into_welcome_by_itself`,
  `motion_zero_shows_the_intros_last_frame`.
- Decision 13, done: the hub takes accounts alone (`chimera_services`'
  accounts give tickets for a device's signed request only, accounts
  migration 3 `0003_key_devices_only.sql` keeps key devices only, the
  hub's asset service runs `TicketAuth` alone); the first run is due
  with no account and no skip mark (`onboarding::first_run_due`). Tests:
  `onboarding::a_pc_with_no_account_sees_the_first_run`,
  `accounts::tickets_are_the_hubs_only_way_in`,
  `db::version_three_keeps_key_devices_only`.
- The intro on a real first launch (2026-11-21): its clock starts once
  the app is warm, not at app start (the first frames are slow, and the
  player missed its first second): `chimera_ui::Warmth` ("start when the
  app is warm", a kit rule any screen can use): warm after 10 frames in a
  row each under 50 ms of real time, or after 6 s of frames on a PC that
  never gets there; the intro stands on its first frame until then
  (`Intro::warming`). The UI clock counts a real frame at most 0.1 s
  (`motion::MAX_STEP`), so one hitch never skips its beats. Tests:
  `chimera_ui` `motion::a_long_frame_is_clamped`,
  `the_app_is_warm_after_steady_frames`,
  `warmth_counts_the_manual_clocks_steps`; `onboarding::
  the_intro_starts_once_the_frames_are_steady`.
- Decision 4, the link handler, done (2026-11-21):
  `chimera_client::client::link_handler`. `chimera-hub://` is
  registered per user with no admin rights, a few frames into each real
  start when it opens something else (first launch, the program moved):
  Windows `HKCU\Software\Classes\chimera-hub` through `reg.exe`
  (`"<exe>" +chimera:onboard "%1"`), Linux `chimera-hub.desktop` in
  `$XDG_DATA_HOME/applications` (`x-scheme-handler/chimera-hub`) made the
  default in `$XDG_CONFIG_HOME/mimeapps.list` (written directly: what
  `xdg-mime default` does). `cl_link_handler 0` opts out; never in a
  `dev` build (unless `CHIMERA_LINK_HANDLER=1`), never with
  `CHIMERA_NO_LINK_HANDLER` (tools/agent-env.sh sets it), a tool run or
  a test. A start by a link takes the link only (`link_launch`, checked
  by `accept`: host, port, optional fingerprint, the code, nothing else),
  and hands it to a running game when one holds the inbox
  (`<data_local>/chimera/links`, a locked `running-<pid>.lock` per game;
  the link a file the running game takes every 0.5 s), else starts with
  it; the first run comes up with the field filled and checked
  (`onboarding::open_links`, `Source::Opened`). The registration is
  behind `SchemeRegistry` (tests: a fake `reg.exe`, temporary XDG
  folders). Tests: `chimera_client` `link_handler::*` (6),
  `party_kit` `onboarding::an_opened_link_fills_the_field_and_checks_it`.
  Not done: macOS; unregistering (by hand).
- Milestone (e), the account page, owner invites and base files up
  front, done 2026-11-21 ("The account page" above). Tests:
  `chimera_services` `accounts::the_account_page_checks_renames_and_signs_out`,
  `an_owner_makes_lists_and_revokes_invites_in_the_game`; `party_kit`
  `account_page::` (the rename with free names, avatar, link code and
  sign out, an owner's invites, every staged screen, the base files
  started at the first run's end with a strip that takes no clicks);
  `ui_scene account <profile|rename|rename-taken|avatar|link|sign-out|
  invites|invite-made|player>`, `ui_scene downloads <planning|running|
  done|failed>`. The accounts service needs deploying for the new calls.
