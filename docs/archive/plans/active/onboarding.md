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
