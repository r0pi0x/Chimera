# Plan: session-only parties, joined by invite code

Status: started 2026-11-17; steps 1-4 built (as built: the section at
the end). Backlog: "Parties and friends: the social
layer" ([../../backlog.md](../../backlog.md)); the user's decisions are
recorded there and followed here. Scope of this plan's first run, in the
user's order: **parties without friends**, joined by an invite code; no
friends list, no presence beyond the party, no party chat yet. Builds on
the hub service ([hub.md](hub.md), S2) and the Signal r8 shell
([ugg-ui.md](ugg-ui.md)).

## What exists

- The lobby's rows carry a party number (`chimera_net::prematch::
  LobbyRow::party`, a `u8`, 0 alone) and the client panel groups and
  edges rows that share one; the server always sends 0.
- The shell's rack shows "PARTY n/8", your avatar and a disabled "+"
  (`chimera_ui::shell`), from `chimera_ui::shell::Party`, which
  `chimera_client::client::profile` writes as you alone.
- The hub service (`chimera_services::hub`, binary `chimera_hub`) lists
  servers; every route needs an account's ticket (hub.md decision 10),
  checked locally; the client's `chimera_net::net::hub::Hub` calls it on
  threads.
- A player's userinfo (`cl_*` cvars marked userinfo) reaches the server
  at the join and on change (`core::UserInfo` on its character); the
  lobby already reads `cl_avatar` from it.

## The model

- **Party**: a public id (`id`, 12 hex digits: what a game server sees,
  grants nothing), an **invite code** (6 Crockford base32 characters
  shown `XXX-XXX`, read back ignoring case, dashes and spaces, O as 0 and
  I/L as 1: `chimera_services::codes`; unique among live parties), a
  **leader** (a member), **members** (at most `MAX_MEMBERS`, 8, the
  rack's 8), and where the leader has taken it (**target**: a game
  server's address and name, or none) with a **move** counter that goes
  up each time the leader takes the party somewhere (a server) or out
  of one ("Leave with my party").
- **Member**: a member id (8 hex digits, the party's own), a display
  name and avatar byte (the player's `name` and `cl_avatar`, sent with
  each call), when it joined, when it was last seen, where it is (`at`:
  the game server it is connected to, or none: the menu), and the hash of
  its **member token**.
- **Member token**: `chimera_party_` and 32 hex digits, made by the hub
  when a member joins or creates, returned once, kept by the client in
  `party.toml` beside `hub.toml` (the user's data folder; `CHIMERA_PARTY`
  names another) with the hub's origin. Every party call carries it in
  the body (never the URL, never logged). It is what makes "closing and
  reopening the game keeps you in the party" work: the next start reads
  it and asks the party's state. The hub keeps its SHA-256 only.
- **Online**: seen in the last `ONLINE_SECS` (45 s). A client asks the
  state every `POLL_SECS` (10 s) while it is in a party, which is the
  heartbeat.
- **Expiry**: a member not seen for `MEMBER_EXPIRY` (6 hours) is
  dropped; a party with no members is gone at once (its code free
  again). A leader offline for `LEAD_HANDOVER` (2 minutes) while another
  member is online hands the lead to the online member who joined first
  (an open question below).

Accounts: every call needs an account's ticket, as every hub route does
(decision 10), so only players signed in to a hub have parties; the
membership itself is the token, not the account (session-only, as the
user ordered). The account id is kept on the member so the step that
ties parties to accounts can find them.

## Where the state lives

On the hub (the user's decision), in memory, written to `hub.db` (table
`parties`, one JSON row per party, written on every change but not on
each poll) so a restart of the hub keeps the parties; a member's last
seen comes back as the restart's time. Tests run the hub in process on
loopback (`chimera_services` tests) and the game's tests use a
`PartyHub` seam (below) with a fake in memory, so no test needs a
running hub.

## The client and the hub

`chimera_assets::party_doc` holds the JSON both sides use and the
blocking calls (as `hub_doc`); the service is `chimera_services::hub::
party`. All POST, JSON, `Authorization: Bearer <ticket>`:

| Call | Body | Answer | Who |
|---|---|---|---|
| `/hub/party/create` | name, avatar | token, the party | anyone (leaves no other party: the client leaves first) |
| `/hub/party/join` | code, name, avatar | token, the party | anyone; 404 a wrong code (counted), 409 full |
| `/hub/party/state` | token, name, avatar, at | the party | a member (the heartbeat) |
| `/hub/party/leave` | token | | a member; the leader's lead passes on |
| `/hub/party/kick` | token, member | the party | the leader |
| `/hub/party/promote` | token, member | the party | the leader |
| `/hub/party/lead` | token, target or none, together | the party | the leader: takes the party to a server (target), or out of one (none, together: "Leave with my party"; none, alone: "Leave alone", nobody moves) |

The party as answered (`PartyView`): id, code, leader, the members (id,
name, avatar, online, at, leader), the target, the move counter and its
kind (`join` or `leave`), and which member you are. A token the hub
doesn't know is 404 ("not in a party": the client forgets it); a kicked
member's token is 404 too.

**Limits** (as the hub's others: a refusal with its status and words):
per account, at most `PARTY_CALLS` (60) party calls a minute (429), a
poll counted like any call; wrong codes per account and per address at
most `CODE_FAILURES` (10) in 10 minutes, then 429 for the rest of it;
names cut to 20 characters, no control characters; at most 8 members.

**A leader who hosts**: `lead` with `hosted` and a port: the hub puts the
address it sees the leader at (a loopback peer: `hub.toml`'s
`public_address`), as heartbeats do; a body never names another machine
for a hosted game.

## The client and the game server

- **The party id to the server**: `cl_party` (userinfo, not saved): the
  party's public id, empty alone. The client's party module sets it from
  the hub's answer.
- **Party numbers**: the server gives each party id with two or more
  players there a small number (1-255, kept while any of it is there:
  `prematch::PartyNumbers`) and puts it in `LobbyRow::party`, so the
  lobby's edges and grouping show. A party of one there is 0.
- **Teams** where the mode has two sides: a joining player goes to the
  side most of its party there is on, else the smaller side (the
  network's pick, `server::spawn_player`, and the mode runtime's
  `mode::assign_team` for one without a side). Free-for-all modes (the
  gun games) have nothing to keep.
- **Following the leader**: the leader's client tells the hub where it
  went (`lead`, after its join is welcomed). Every member's client reads
  the move from its next poll; a move it has not acted on:
  - *join*: on the menu (not connected), it joins at once ("Joining your
    party's server" in the status line); in a match elsewhere, a prompt
    "Your party went to <name>: Join / Stay" (Join is `connect`).
    Already there: nothing.
  - *leave*: connected to the server the party left, it goes back to the
    menu with the note "Your party's leader left".
  A map change keeps every client connected, so the party stays
  together across map changes and matches with no message.

## Leaving (the user's decisions)

- **Back to the menu** (Esc, Disconnect, the lobby's Leave): you stay in
  the party; you can rejoin at will (the party's server is shown on the
  party panel with Join).
- **Closing and reopening the game**: the token in `party.toml` keeps
  you in; the next start polls the party at once.
- **Leave the party**: an explicit button on the party panel (and the
  console's `party_leave`).
- **The leader leaving a match** (in a party of two or more): Leave's
  confirm offers **Leave with my party** (the default, first, Enter) and
  **Leave alone**. With: `lead` none, together (the members' clients go
  back to the menu), then `disconnect`. Alone: `lead` none, alone, then
  `disconnect`; the members stay.

## The UI (provisional)

Built from the existing kit components only; **no new visual design:
the user signs off designs, so all of this is provisional** until a
design replaces it.

- **The rack**: the party's avatars after yours (the leader's strip
  mark, the away and idle marks from `online`), "PARTY n/8". The "+"
  is live when the client has a hub (an account), else disabled with
  "Sign in to a hub first".
- **The party panel** (the "+", or a party member's avatar): a
  centered dialog with the kit's dialog parts: not in a party, *Create a
  party* and *Join with a code* (a text field, Join); in one, the
  invite code large with *Copy*, the members (leader marked; the
  leader's *Promote* and *Kick* per member), where the party is (with
  *Join* when you're not there) and *Leave party*.
- **The follow prompt** and the leader's two-choice Leave confirm as
  above.
- Console for every action (tests, screenshots): `party`,
  `party_create`, `party_join <code>`, `party_leave`, `party_kick <id>`,
  `party_promote <id>`, `party_panel [0|1]`.

## Steps

1. This plan, linked from the backlog.
2. The hub: `party_doc`, `hub::party` (the model, the limits, the
   table), the routes; tests on the in-process hub through the front.
3. The game: `chimera_net::net::party` (the client's party: token file,
   polls on a thread through the `PartyHub` seam, `cl_party`, following
   and leaving), the server's party numbers and teams; NetSim scenario
   tests with a fake hub.
4. The provisional UI: the rack, the party panel, the follow prompt, the
   leader's Leave confirm; offscreen screenshots.

Later: party chat (Enter opens chat, Tab cycles all, team, party: the
user's decisions), friends and presence (accounts, hub.md S3/S4),
invites from Friends, the designed party panel.

## Open questions for the user

The conservative choice is built until the user says otherwise.

1. **Parties need a hub account** (decision 10: no anonymous hub
   access). Players with no account (LAN only) have no parties. Built:
   account needed; the "+" says so.
2. **A leader who goes offline**: built: after 2 minutes offline the lead
   passes to the online member who joined first; the old leader comes
   back as a member.
3. **Following into a full server**: built: the member's join is
   refused as any join is, and the panel keeps the party's server with
   Join to try again. No reserved slots.
4. **Auto-join from the menu**: built: a member on the menu follows at
   once; a member in another match is asked. Should the menu ask too?
5. **Party size**: 8, the rack's. **Teams**: a party is kept on one side
   even when that unbalances the sides (no cap).
6. **The party id a client claims is not checked by the game server**
   (it only groups rows and picks a side). A signed party pass from the
   hub would close it, if it matters.
7. **How long an idle member stays**: 6 hours unseen, then dropped.

## As built (2026-11-17)

- **The hub**: `chimera_assets::party_doc` (the JSON, the calls,
  `party.toml`), `chimera_services::hub::party` (the model, the limits,
  the lead handover, expiry; table `parties`, hub.db version 2), the
  routes in `hub::http`. Tests: `chimera_services` `hub::parties::`.
- **The game**: `chimera_net::net::party` (`PartyClient`, the
  `PartyHub` seam with `HttpPartyHub`, `cl_party`, the follow rule
  `follow`, `PartyGo` acted on as `connect`/`disconnect`, the console
  commands), `use_user_party` at the party apps' start after the hub;
  the server's `prematch::PartyNumbers` and `LobbyRow::party`; teams by
  `core::party_side` in `server::spawn_player` and `mode::assign_team`.
  Tests: `chimera_net` `parties::` (sides, lobby numbers, `cl_party`
  changes) and `hub::parties::` (two clients with the real services: made
  by code, grouped on the server, following the leader in, leaving with
  it, leaving the party); the follow rule's unit test.
- **The UI (provisional)**: the rack's "+" live with a hub account
  (`chimera_ui::shell::Party::invite`), the party's avatars in the rack
  (`client::profile`), the party panel and the follow prompt
  (`party_kit::party_signal`, `ui_scene party [in|lead|prompt]`), the
  leader's Leave confirm in the Escape menu and over the lobby
  (`esc_menu::Page::ConfirmPartyLeave`: Leave with my party first, Leave
  alone, Stay).
- Not built: party chat; Copy for the code; the designed panel; a hub
  admin view of parties.
