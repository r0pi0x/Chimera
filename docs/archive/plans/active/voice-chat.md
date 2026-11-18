# Plan: voice chat (`+voicerecord`)

Status: started 2026-10-15. Parity rows: docs/plans/active/parity-gaps.md
("Voice chat", "`+voicerecord` (voice chat), Voice tab").

Lucker Party is played online with friends: push-to-talk voice in the
game, CS:S's way (K held talks; teammates hear you; the dead talk to the
dead), instead of Discord on the side.

## Layers

| Part | Where | Why there |
|---|---|---|
| Wire messages, the server's relay, who hears whom (a hook), `sv_voiceenable`, who is talking (`VoiceActivity`) | `chimera_net` (`net::voice`) | The transport is the engine's; the dedicated server relays without a codec or audio device (it links no `chimera_client`). |
| Capture and playback devices (behind traits, with fakes), the Opus codec, `+voicerecord`, `voice_*` client cvars, the jitter buffer and mixer | `chimera_client` (`client::voice`) | Only a client with a window and sound has a microphone and speakers; the server never decodes. |
| The hearing rule (`sv_alltalk`: teams, the dead), the HUD (`HudVoiceSelfStatus`, `HudVoiceStatus` from `scripts/hudlayout.res`, the icons `voice_self` and `voice_player` from `scripts/hud_textures.txt`), K bound to `+voicerecord` | `cs_source` (`game::net` for the rule, `ui::voice_hud` for the HUD) | CS:S's rules and look. The bind was already CS:S's default. |

## Wire format (`NET_VERSION` net25)

- Client -> server `VoiceData { seq: u16, frames: Vec<Vec<u8>> }`, the
  unreliable channel: the Opus packets encoded since the last frame
  (each 20 ms of 48 kHz mono), `seq` the first one's number. Nothing
  is sent while nobody talks, so the bandwidth budget and
  `heavy::net_capacity` see no change at rest.
- Server -> each listener `VoiceRelay { speaker: Entity, seq, frames }`,
  unreliable, the speaker mapped. A lost packet is a gap the decoder
  fills (Opus's loss concealment), as Source's voice is unreliable too.
- The server checks: `sv_voiceenable`, the sender has a character, at
  most 8 frames and 1000 bytes a frame per message, and a byte allowance
  per speaker (16 kB burst, 8 kB/s: three times Opus's ~3.7 kB/s at
  24 kbit/s). Then `VoiceRules::can_hear(listener, talker)` for every
  player (the listen server's host included) but the talker.

## Codec

Opus (48 kHz mono, 20 ms frames, VoIP mode, 24 kbit/s, wideband: its
SILK layer only, since `mousiki`'s CELT layer, used by wider bands,
trips one of its own debug assertions): CS:S's
`sv_voicecodec` is `steam` on the probe server (Steam's voice API, which
is Opus-based; the legacy choices are Speex and CELT), so Opus is the
faithful choice. Crate: `mousiki` 0.2 (MIT), a pure-Rust port of the
reference libopus: encoder and decoder, no C compiler or system package,
so it builds on Linux and Windows alike. Measured here: 1 s of speech-like
audio encodes and decodes in ~13 ms (release), ~3.7 kB/s.
(`unsafe-libopus`, a c2rust transpile, and `opus-head-sys`, which builds
libopus with `cc`, were the alternatives.)

## Devices

`cpal` 0.17 (already in the tree under rodio, which Bevy's audio uses):
the default input device for the microphone, the default output device
for a small mixer of the talkers' streams (their own stream, not Bevy's
audio, so a talker's samples go straight to the device's callback). Each
device runs on its own thread, which owns the stream (cpal's streams
aren't `Send` everywhere). Resampled linearly to and from 48 kHz,
down-mixed to mono and spread to the device's channels. Not positional:
CS:S plays voice flat (no 3D voice cvar in CS:S).

Behind traits (`VoiceInput`, `VoiceOutput`) with fakes (`FakeInput`, a
queue of samples; `FakeOutput`, what each talker played), so tests run
headless.

## Cvars

| Cvar | Default | Source of the default |
|---|---|---|
| `sv_voiceenable` | 1 | measured (probe server, 2026-10-15) |
| `sv_alltalk` | 0 | measured (probe server); "Players can hear all other players, no team restrictions" |
| `sv_voicecodec` | (not a cvar here) | probe: `steam`; ours is always Opus |
| `voice_enable` | 1 | guess (client cvar, not on the dedicated server): voice heard and sent |
| `voice_modenable` | 1 | guess: the Voice tab's "Enable voice in this game" |
| `voice_scale` | 1 | guess: the received voices' volume |
| `voice_loopback` | 0 | guess: hear yourself (ours: decoded locally, not echoed by the server) |
| `voice_vox` | 0 | guess: open microphone (Source has the cvar; its default not captured) |
| `voice_threshold` | 4000 | ours: a 20 ms frame's loudest sample (of 32767) that opens the open microphone's gate, held `VOX_HANG` (0.5 s, ours) after the last such frame |
| `voice_boost` | 0 | ours: the Voice tab's "Boost microphone gain" (x4) |
| `voice_inputfromfile` | 0 | measured on the probe server (exists, 0); not implemented (the fake input does the tests' part) |

The client cvars' CS:S defaults are open questions for a Windows capture
(`docs/plans/active/windows-captures.md`).

## Who hears whom (CS:S, `sv_alltalk 0`)

A living talker: every teammate (alive or dead). A dead talker: only
dead teammates. Spectators are a team of their own (dead). `sv_alltalk 1`:
everyone hears everyone. Measured: only the cvar and its help text;
the dead/alive rule is CS:S's documented behaviour (the brief, community
docs), to confirm with two clients on the probe server.

## Steps

1. Plan (this file).
2. `chimera_net::net::voice`: messages, relay, `VoiceRules` hook,
   `VoiceActivity`, `sv_voiceenable`; `NET_VERSION` net25.
3. `chimera_client::client::voice`: devices, codec, jitter buffer and
   mixer, `+voicerecord`, `voice_*` cvars.
4. `cs_source`: `sv_alltalk` and the rule; the HUD icons and panel.
5. Tests: the rule (unit), the codec (unit), a NetSim scenario (a fake
   microphone on one client heard by a teammate and not an enemy with
   `sv_alltalk 0`, by both with 1).
6. Docs: module docs, ARCHITECTURE.md, parity-gaps rows, backlog.

## Follow-ups (2026-10-16)

1. Per-player mute (done): the game menu's Player List (CS:S's
   `CPlayerListDialog`, the install's `PlayerListDialog.res`; in a game,
   under Disconnect): select a player, "Mute in-game voice" (the
   default button) runs `voice_mute <userid>`, the row's properties say
   "Muted" and the button unmutes. `net::voice::VoiceMutes` keeps the
   user ids (`NetCharacter::owner`) for the session; a muted talker's
   relay is dropped on the client (not decoded, played or shown in the
   HUD). CS:S's own scoreboard (`scoreboard.res`) has no mute control,
   so the dialog is the way, as in CS:S. Not as CS:S: the server still
   sends the muted voice (Source's `vban` stops it there) and mutes
   aren't saved (CS:S keeps them by Steam id in `voice_ban.dt`).

2. Open microphone (done): `voice_vox 1` keeps the microphone open;
   a frame whose loudest sample reaches `voice_threshold` opens the
   gate for `VOX_HANG` of audio (counted in frames, so steady at any
   frame rate); frames under it while closed are dropped, the key still
   talks. The HUD's own icon shows while the gate is open.

3. The Voice tab (done): Test Microphone runs `voice_test` (ours): the
   microphone open, each frame encoded and decoded locally and played
   back, nothing sent, "Stop Microphone Test" while it runs, stopped
   when the tab or the dialog closes. The meter (`MicMeter`,
   `MicMeter2` from `OptionsSubVoice.res`: the install's dark and lit
   bar pictures) lights `barCount` bars for `client::voice::VoiceLevel`
   (a frame's loudest sample, on a decibel scale from -48 dB, falling
   smoothly; ours). "Boost microphone gain" is `voice_boost` (ours: x4,
   +12 dB; CS:S's sets the sound card's boost). The transmit volume
   slider (the layout's `#GameUI_MicrophoneVolume` panel) is
   `voice_transmit`, 0 to 1 times the samples (ours; CS:S's sets the sound
   card's microphone level).

## Left for later

- Talking icons over heads (`voice/icntlk_pl`), Steam avatars in the
  voice panel.
- `voice_inputfromfile`, `voice_recordtofile`.
