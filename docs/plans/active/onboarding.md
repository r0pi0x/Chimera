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

## The flow (client)

1. **First launch:** a short title moment with motion (the kit's
   `chimera_ui::motion`, slowed or off by the motion setting). Then one
   screen with the redeem field as the hero ("Paste your invite"), a
   secondary "I already have an account", and a small "Play without an
   account" link.
2. **Redeem:** the one field. While checking it says "Reaching the
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
