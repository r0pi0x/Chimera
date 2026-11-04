// chimera_probe (HL2:DM): replays per-tick inputs on a bot in a real
// Half-Life 2: Deathmatch dedicated server and logs its state after every
// tick, so Chimera's HL2:DM movement, suit, weapons and projectiles can be
// compared with the game tick for tick (tools/hl2dm_probe/README.md; the
// CS:S probe's format, tools/css_probe/mashup_probe.sp, plus the suit's,
// the weapon's and tracked entities' numbers).
//
// The bot must not think: an `hl2mp_bot_add` bot's NextBot behaviour
// crashes the server when it dies (its update reads a stale action), so
// the probe runs with `nb_player_stop 1` (probe.py's `setup` sets it);
// the bot still runs the commands this plugin writes.
//
// Server commands:
//   probe_run <input file> <output file>   (paths relative to hl2mp/):
//     runs on the driven bot (probe_bot, else the first living fake
//     client) and keeps running across its death.
//     Input: one optional "start x y z pitch yaw vx vy vz" line, then one
//     line per tick "buttons forwardmove sidemove pitch yaw [select
//     [impulse]]" (select: a weapon class the bot switches to, or "-"),
//     and any number of "cmd <row> <client> <command ...>" lines: on that
//     input row (before its input) the client runs the command
//     (FakeClientCommand), or the server when client is 0.
//     Output, per tick (tick 0: placed):
//       "tick x y z vx vy vz onground ducked hull_top eye buttons water
//        movetype aux health armor maxspeed life observer_mode gametime
//        server_tick tick_base" (gametime: the command's time, from the
//        bot's tick base, which need not be the server's tick)
//       "W tick class clip1 ammo1 ammo2 next_primary next_secondary"
//         (the active weapon; the next-attack times relative to gametime)
//       "E tick index class x y z vx vy vz next_think_tick server_tick
//        effects pitch yaw roll" (tracked entities; effects bit 32: not
//        drawn, e.g. a taken item; the angles: the absolute rotation)
//     and as they happen: "S tick channel volume level pitch sample entry"
//     (the bot's sounds; "s ..." the same for tracked entities),
//     "H tick health armor" (player_hurt), "D tick victim attacker
//     inflictor damage type health" (damage taken by players and tracked
//     entities), "A tick victim attacker damage type hitbox hitgroup"
//     (a player's traced hit, before the hitgroup scale), "C tick index class" / "X tick index class" (tracked
//     entities created / removed), "T tick name ..." (bullets: "Shotgun
//     Shot" origin, direction, ammo, seed, shots, spread; EffectDispatch:
//     effect name, origin), "V tick event client [attacker | team
//     oldteam]" (player_death with the attacker's client, player_spawn,
//     player_team with the new and old team).
//   probe_bot <client>: drive this client (0: the first living bot).
//   probe_set <health> <armor> [aux]: the bot's health, armour, aux power.
//   probe_give <class>: give the bot an item or weapon.
//   probe_ammo <type> <count>: the bot's ammo of a type index.
//   probe_strip: remove the bot's weapons.
//   probe_inv: the bot's weapons (clips, ammo types) and ammo.
//   probe_kill: kill the bot (suicide).
//   probe_cmd <command>: the bot runs a client command (FakeClientCommand).
//   probe_hurt <damage> <type> [attacker client]: damage the bot.
//   probe_tp <client> <x y z pitch yaw>: place any player.
//   probe_find <class substring>: entities' index, class, name, origin.
//   probe_prop <class> <model> <x y z> [mass [spawnflags]]: spawn a
//     physics prop, tracked; prints its index (spawnflags 72: motion
//     disabled until the gravity gun touches it, so it hangs in the air).
//   probe_track <index> | clear | <class prefix>: track an entity, or
//     every new entity whose class starts with the prefix.
//   probe_get <index> <data|send> <prop> [int|float|vec|ent]: read a prop.
//   probe_setprop <index> <data|send> <prop> <int|float> <value>.
//   probe_fire <index> <input> [value]: AcceptEntityInput.
//   probe_water <x0 y0 z0> <x1 y1 z1> <step> <depth>: grid points (the
//     lowest per column) in water with water `depth` units above too.
//   probe_info: every living player's position and angles.
//   probe_score: every player's team, frags, deaths and life state, and
//     each team's score.

#include <sourcemod>
#include <sdktools>
#include <sdkhooks>

public Plugin myinfo = {
    name = "chimera probe (HL2:DM)",
    author = "chimera",
    description = "Replays per-tick inputs on a bot and logs its movement, suit, weapon and nearby entities",
    version = "3",
};

#define MAX_TICKS 16384
#define MAX_TRACK 64
#define MAX_PREFIX 16

int g_buttons[MAX_TICKS];
float g_forward[MAX_TICKS];
float g_side[MAX_TICKS];
float g_pitch[MAX_TICKS];
float g_yaw[MAX_TICKS];
int g_impulse[MAX_TICKS];
char g_select[MAX_TICKS][32];
int g_count;
#define MAX_CMDS 64
int g_cmdRow[MAX_CMDS];
int g_cmdClient[MAX_CMDS];
char g_cmdText[MAX_CMDS][96];
int g_cmdCount;
float g_startPos[3];
float g_startAng[3];
float g_startVel[3];
bool g_hasStart;
int g_bot;
int g_wantBot;
File g_out;
int g_cursor = -1;
int g_track[MAX_TRACK];
int g_trackCount;
char g_prefix[MAX_PREFIX][64];
int g_prefixCount;

public void OnPluginStart()
{
    AddNormalSoundHook(SoundHook);
    HookEvent("player_hurt", Event_Hurt);
    HookEvent("player_death", Event_Life);
    HookEvent("player_spawn", Event_Life);
    HookEvent("player_team", Event_Team);
    AddTempEntHook("Shotgun Shot", TE_Bullets);
    AddTempEntHook("EffectDispatch", TE_Effect);
    RegServerCmd("probe_run", Command_Run, "probe_run <input> <output>");
    RegServerCmd("probe_bot", Command_Bot, "probe_bot <client>");
    RegServerCmd("probe_set", Command_Set, "probe_set <health> <armor> [aux]");
    RegServerCmd("probe_give", Command_Give, "probe_give <class>");
    RegServerCmd("probe_ammo", Command_Ammo, "probe_ammo <type> <count>");
    RegServerCmd("probe_strip", Command_Strip, "probe_strip");
    RegServerCmd("probe_kill", Command_Kill, "probe_kill");
    RegServerCmd("probe_water", Command_Water, "probe_water <x0 y0 z0> <x1 y1 z1> <step> <depth>");
    RegServerCmd("probe_cmd", Command_Cmd, "probe_cmd <client command>");
    RegServerCmd("probe_inv", Command_Inv, "probe_inv: the bot's weapons and ammo");
    RegServerCmd("probe_hurt", Command_Hurt, "probe_hurt <damage> <type> [attacker]");
    RegServerCmd("probe_tp", Command_Tp, "probe_tp <client> <x y z pitch yaw>");
    RegServerCmd("probe_find", Command_Find, "probe_find <class substring>");
    RegServerCmd("probe_prop", Command_Prop, "probe_prop <class> <model> <x y z> [mass [spawnflags]]");
    RegServerCmd("probe_track", Command_Track, "probe_track <index> | clear | <class prefix>");
    RegServerCmd("probe_get", Command_Get, "probe_get <index> <data|send> <prop> [int|float|vec|ent]");
    RegServerCmd("probe_setprop", Command_SetProp, "probe_setprop <index> <data|send> <prop> <int|float> <value>");
    RegServerCmd("probe_fire", Command_Fire, "probe_fire <index> <input> [value]");
    RegServerCmd("probe_info", Command_Info, "probe_info: living players' positions and angles");
    RegServerCmd("probe_score", Command_Score, "probe_score: players' teams, frags, deaths; team scores");
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i)) {
            OnClientPutInServer(i);
        }
    }
}

public void OnClientPutInServer(int client)
{
    // A bot kept across a map change is put in the server again: hook once.
    SDKUnhook(client, SDKHook_OnTakeDamagePost, DamagePost);
    SDKUnhook(client, SDKHook_TraceAttack, TraceAttack);
    SDKHook(client, SDKHook_OnTakeDamagePost, DamagePost);
    SDKHook(client, SDKHook_TraceAttack, TraceAttack);
}

public Action TraceAttack(int victim, int &attacker, int &inflictor, float &damage, int &damagetype, int &ammotype,
    int hitbox, int hitgroup)
{
    if (g_out != null) {
        g_out.WriteLine("A %d %d %d %.4f %d %d %d", g_cursor, victim, attacker, damage, damagetype, hitbox, hitgroup);
    }
    return Plugin_Continue;
}

int FindBot()
{
    if (g_wantBot > 0 && g_wantBot <= MaxClients && IsClientInGame(g_wantBot)) {
        return g_wantBot;
    }
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i) && IsFakeClient(i) && IsPlayerAlive(i)) {
            return i;
        }
    }
    return 0;
}

int RequireBot()
{
    int bot = FindBot();
    if (bot == 0) {
        PrintToServer("probe: no bot");
    }
    return bot;
}

float Aux(int client)
{
    if (HasEntProp(client, Prop_Send, "m_flSuitPower")) {
        return GetEntPropFloat(client, Prop_Send, "m_flSuitPower");
    }
    return -1.0;
}

PropType Kind(const char[] s)
{
    return StrEqual(s, "send") ? Prop_Send : Prop_Data;
}

bool Tracked(int entity)
{
    for (int k = 0; k < g_trackCount; k++) {
        if (g_track[k] == entity) {
            return true;
        }
    }
    return false;
}

void Track(int entity)
{
    if (!Tracked(entity) && g_trackCount < MAX_TRACK) {
        g_track[g_trackCount++] = entity;
        SDKHook(entity, SDKHook_OnTakeDamagePost, DamagePost);
    }
}

public Action Command_Bot(int args)
{
    g_wantBot = GetCmdArgInt(1);
    PrintToServer("probe: driving %d", FindBot());
    return Plugin_Handled;
}

public Action Command_Set(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    SetEntityHealth(bot, GetCmdArgInt(1));
    SetEntProp(bot, Prop_Data, "m_ArmorValue", GetCmdArgInt(2));
    if (args >= 3 && HasEntProp(bot, Prop_Send, "m_flSuitPower")) {
        SetEntPropFloat(bot, Prop_Send, "m_flSuitPower", GetCmdArgFloat(3));
    }
    PrintToServer("probe: set %d health %d armor %d aux %.3f", bot, GetClientHealth(bot),
        GetEntProp(bot, Prop_Data, "m_ArmorValue"), Aux(bot));
    return Plugin_Handled;
}

public Action Command_Give(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    char cls[64];
    GetCmdArg(1, cls, sizeof(cls));
    PrintToServer("probe: gave %s: %d", cls, GivePlayerItem(bot, cls));
    return Plugin_Handled;
}

public Action Command_Ammo(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    int type = GetCmdArgInt(1);
    SetEntProp(bot, Prop_Send, "m_iAmmo", GetCmdArgInt(2), _, type);
    PrintToServer("probe: ammo %d = %d", type, GetEntProp(bot, Prop_Send, "m_iAmmo", _, type));
    return Plugin_Handled;
}

public Action Command_Strip(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    int n = GetEntPropArraySize(bot, Prop_Send, "m_hMyWeapons");
    for (int k = 0; k < n; k++) {
        int w = GetEntPropEnt(bot, Prop_Send, "m_hMyWeapons", k);
        if (w > 0 && IsValidEdict(w)) {
            RemovePlayerItem(bot, w);
            AcceptEntityInput(w, "Kill");
        }
    }
    PrintToServer("probe: stripped %d", bot);
    return Plugin_Handled;
}

public Action Command_Inv(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    int n = GetEntPropArraySize(bot, Prop_Send, "m_hMyWeapons");
    char cls[32];
    char model[PLATFORM_MAX_PATH];
    GetClientModel(bot, model, sizeof(model));
    PrintToServer("model %s team %d", model, GetClientTeam(bot));
    for (int k = 0; k < n; k++) {
        int w = GetEntPropEnt(bot, Prop_Send, "m_hMyWeapons", k);
        if (w > 0 && IsValidEdict(w)) {
            GetEdictClassname(w, cls, sizeof(cls));
            PrintToServer("weapon %d %s clip %d clip2 %d ammo type %d", w, cls, GetEntProp(w, Prop_Send, "m_iClip1"),
                GetEntProp(w, Prop_Send, "m_iClip2"), GetEntProp(w, Prop_Send, "m_iPrimaryAmmoType"));
        }
    }
    int types = GetEntPropArraySize(bot, Prop_Send, "m_iAmmo");
    for (int t = 0; t < types; t++) {
        int a = GetEntProp(bot, Prop_Send, "m_iAmmo", _, t);
        if (a != 0) {
            PrintToServer("ammo %d = %d", t, a);
        }
    }
    return Plugin_Handled;
}

public Action Command_Cmd(int args)
{
    int bot = RequireBot();
    if (bot != 0) {
        char text[256];
        GetCmdArgString(text, sizeof(text));
        FakeClientCommand(bot, "%s", text);
        PrintToServer("probe: %d ran %s", bot, text);
    }
    return Plugin_Handled;
}

public Action Command_Water(int args)
{
    float lo[3];
    float hi[3];
    for (int k = 0; k < 3; k++) {
        lo[k] = GetCmdArgFloat(1 + k);
        hi[k] = GetCmdArgFloat(4 + k);
    }
    float step = GetCmdArgFloat(7);
    float depth = GetCmdArgFloat(8);
    int found = 0;
    float p[3];
    float q[3];
    for (p[0] = lo[0]; p[0] <= hi[0] && found < 20; p[0] += step) {
        for (p[1] = lo[1]; p[1] <= hi[1] && found < 20; p[1] += step) {
            for (p[2] = lo[2]; p[2] <= hi[2] && found < 20; p[2] += step) {
                q[0] = p[0];
                q[1] = p[1];
                q[2] = p[2] + depth;
                if ((TR_GetPointContents(p) & CONTENTS_WATER) && (TR_GetPointContents(q) & CONTENTS_WATER)) {
                    PrintToServer("water %.0f %.0f %.0f", p[0], p[1], p[2]);
                    found++;
                    break;
                }
            }
        }
    }
    PrintToServer("probe: %d water points", found);
    return Plugin_Handled;
}

public Action Command_Kill(int args)
{
    int bot = RequireBot();
    if (bot != 0) {
        ForcePlayerSuicide(bot);
        PrintToServer("probe: killed %d", bot);
    }
    return Plugin_Handled;
}

public Action Command_Hurt(int args)
{
    int bot = RequireBot();
    if (bot == 0) {
        return Plugin_Handled;
    }
    int attacker = args >= 3 ? GetCmdArgInt(3) : 0;
    SDKHooks_TakeDamage(bot, attacker, attacker, GetCmdArgFloat(1), GetCmdArgInt(2), -1, NULL_VECTOR, NULL_VECTOR,
        false);
    PrintToServer("probe: hurt %d health %d armor %d", bot, GetClientHealth(bot),
        GetEntProp(bot, Prop_Data, "m_ArmorValue"));
    return Plugin_Handled;
}

public Action Command_Tp(int args)
{
    int c = GetCmdArgInt(1);
    float o[3];
    float a[3];
    for (int k = 0; k < 3; k++) {
        o[k] = GetCmdArgFloat(2 + k);
    }
    a[0] = GetCmdArgFloat(5);
    a[1] = GetCmdArgFloat(6);
    float zero[3];
    TeleportEntity(c, o, a, zero);
    PrintToServer("probe: placed %d", c);
    return Plugin_Handled;
}

public Action Command_Find(int args)
{
    char want[64];
    GetCmdArg(1, want, sizeof(want));
    char cls[64];
    char name[64];
    for (int e = MaxClients + 1; e < GetMaxEntities(); e++) {
        if (!IsValidEdict(e)) {
            continue;
        }
        GetEdictClassname(e, cls, sizeof(cls));
        if (StrContains(cls, want) < 0) {
            continue;
        }
        name[0] = '\0';
        if (HasEntProp(e, Prop_Data, "m_iName")) {
            GetEntPropString(e, Prop_Data, "m_iName", name, sizeof(name));
        }
        float o[3];
        float a[3];
        if (HasEntProp(e, Prop_Data, "m_vecAbsOrigin")) {
            GetEntPropVector(e, Prop_Data, "m_vecAbsOrigin", o);
        }
        if (HasEntProp(e, Prop_Data, "m_angAbsRotation")) {
            GetEntPropVector(e, Prop_Data, "m_angAbsRotation", a);
        }
        PrintToServer("%d %s '%s' %.2f %.2f %.2f angles %.1f %.1f %.1f", e, cls, name, o[0], o[1], o[2], a[0], a[1],
            a[2]);
    }
    return Plugin_Handled;
}

public Action Command_Prop(int args)
{
    char cls[64];
    char model[PLATFORM_MAX_PATH];
    GetCmdArg(1, cls, sizeof(cls));
    GetCmdArg(2, model, sizeof(model));
    float o[3];
    for (int k = 0; k < 3; k++) {
        o[k] = GetCmdArgFloat(3 + k);
    }
    int e = CreateEntityByName(cls);
    if (e < 0) {
        PrintToServer("probe: cannot create %s", cls);
        return Plugin_Handled;
    }
    PrecacheModel(model);
    DispatchKeyValue(e, "model", model);
    if (args >= 6) {
        char script[64];
        char mass[16];
        GetCmdArg(6, mass, sizeof(mass));
        Format(script, sizeof(script), "mass,%s", mass);
        DispatchKeyValue(e, "overridescript", script);
    }
    if (args >= 7) {
        char flags[16];
        GetCmdArg(7, flags, sizeof(flags));
        DispatchKeyValue(e, "spawnflags", flags);
    }
    DispatchKeyValueVector(e, "origin", o);
    DispatchSpawn(e);
    ActivateEntity(e);
    Track(e);
    PrintToServer("probe: prop %d", e);
    return Plugin_Handled;
}

public Action Command_Track(int args)
{
    char arg[64];
    GetCmdArg(1, arg, sizeof(arg));
    if (StrEqual(arg, "clear")) {
        g_trackCount = 0;
        g_prefixCount = 0;
    } else if (IsCharNumeric(arg[0])) {
        Track(StringToInt(arg));
    } else if (g_prefixCount < MAX_PREFIX) {
        strcopy(g_prefix[g_prefixCount++], sizeof(g_prefix[]), arg);
    }
    PrintToServer("probe: tracking %d entities, %d prefixes", g_trackCount, g_prefixCount);
    return Plugin_Handled;
}

public Action Command_Get(int args)
{
    int e = GetCmdArgInt(1);
    char kind[8];
    char prop[64];
    char kindOf[8] = "int";
    GetCmdArg(2, kind, sizeof(kind));
    GetCmdArg(3, prop, sizeof(prop));
    if (args >= 4) {
        GetCmdArg(4, kindOf, sizeof(kindOf));
    }
    if (!IsValidEdict(e) || !HasEntProp(e, Kind(kind), prop)) {
        PrintToServer("probe: no %s on %d", prop, e);
    } else if (StrEqual(kindOf, "float")) {
        PrintToServer("%s = %.4f", prop, GetEntPropFloat(e, Kind(kind), prop));
    } else if (StrEqual(kindOf, "vec")) {
        float v[3];
        GetEntPropVector(e, Kind(kind), prop, v);
        PrintToServer("%s = %.4f %.4f %.4f", prop, v[0], v[1], v[2]);
    } else if (StrEqual(kindOf, "ent")) {
        PrintToServer("%s = %d", prop, GetEntPropEnt(e, Kind(kind), prop));
    } else {
        PrintToServer("%s = %d", prop, GetEntProp(e, Kind(kind), prop));
    }
    return Plugin_Handled;
}

public Action Command_SetProp(int args)
{
    int e = GetCmdArgInt(1);
    char kind[8];
    char prop[64];
    char kindOf[8];
    GetCmdArg(2, kind, sizeof(kind));
    GetCmdArg(3, prop, sizeof(prop));
    GetCmdArg(4, kindOf, sizeof(kindOf));
    if (StrEqual(kindOf, "float")) {
        SetEntPropFloat(e, Kind(kind), prop, GetCmdArgFloat(5));
    } else {
        SetEntProp(e, Kind(kind), prop, GetCmdArgInt(5));
    }
    PrintToServer("probe: set %s on %d", prop, e);
    return Plugin_Handled;
}

public Action Command_Fire(int args)
{
    int e = GetCmdArgInt(1);
    char input[64];
    char value[64];
    GetCmdArg(2, input, sizeof(input));
    if (args >= 3) {
        GetCmdArg(3, value, sizeof(value));
        SetVariantString(value);
    }
    PrintToServer("probe: %s on %d: %d", input, e, AcceptEntityInput(e, input));
    return Plugin_Handled;
}

public Action Command_Info(int args)
{
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i) && IsPlayerAlive(i)) {
            float o[3];
            float a[3];
            GetClientAbsOrigin(i, o);
            GetClientEyeAngles(i, a);
            PrintToServer("player %d fake %d at %.2f %.2f %.2f angles %.2f %.2f aux %.3f health %d", i,
                IsFakeClient(i), o[0], o[1], o[2], a[0], a[1], Aux(i), GetClientHealth(i));
        }
    }
    return Plugin_Handled;
}

public Action Command_Run(int args)
{
    char inPath[PLATFORM_MAX_PATH];
    char outPath[PLATFORM_MAX_PATH];
    GetCmdArg(1, inPath, sizeof(inPath));
    GetCmdArg(2, outPath, sizeof(outPath));
    File f = OpenFile(inPath, "r");
    if (f == null) {
        PrintToServer("probe: cannot open %s", inPath);
        return Plugin_Handled;
    }
    g_count = 0;
    g_cmdCount = 0;
    g_hasStart = false;
    char line[256];
    char parts[10][32];
    while (f.ReadLine(line, sizeof(line))) {
        TrimString(line);
        if (line[0] == '\0') {
            continue;
        }
        int n = ExplodeString(line, " ", parts, sizeof(parts), sizeof(parts[]));
        if (StrEqual(parts[0], "cmd") && n >= 4 && g_cmdCount < MAX_CMDS) {
            g_cmdRow[g_cmdCount] = StringToInt(parts[1]);
            g_cmdClient[g_cmdCount] = StringToInt(parts[2]);
            // The command: everything after the third space.
            int at = 0;
            for (int k = 0; k < 3; k++) {
                at += StrContains(line[at], " ") + 1;
            }
            strcopy(g_cmdText[g_cmdCount], sizeof(g_cmdText[]), line[at]);
            g_cmdCount++;
        } else if (StrEqual(parts[0], "start") && n >= 9) {
            for (int k = 0; k < 3; k++) {
                g_startPos[k] = StringToFloat(parts[1 + k]);
                g_startVel[k] = StringToFloat(parts[6 + k]);
            }
            g_startAng[0] = StringToFloat(parts[4]);
            g_startAng[1] = StringToFloat(parts[5]);
            g_startAng[2] = 0.0;
            g_hasStart = true;
        } else if (n >= 5 && g_count < MAX_TICKS) {
            g_buttons[g_count] = StringToInt(parts[0]);
            g_forward[g_count] = StringToFloat(parts[1]);
            g_side[g_count] = StringToFloat(parts[2]);
            g_pitch[g_count] = StringToFloat(parts[3]);
            g_yaw[g_count] = StringToFloat(parts[4]);
            g_select[g_count][0] = '\0';
            if (n >= 6 && !StrEqual(parts[5], "-")) {
                strcopy(g_select[g_count], sizeof(g_select[]), parts[5]);
            }
            g_impulse[g_count] = n >= 7 ? StringToInt(parts[6]) : 0;
            g_count++;
        }
    }
    delete f;
    g_bot = FindBot();
    if (g_bot == 0) {
        PrintToServer("probe: no bot");
        return Plugin_Handled;
    }
    if (g_out != null) {
        delete g_out;
    }
    g_out = OpenFile(outPath, "w");
    if (g_out == null) {
        PrintToServer("probe: cannot write %s", outPath);
        return Plugin_Handled;
    }
    g_cursor = -1;
    PrintToServer("probe: running %d ticks on client %d", g_count, g_bot);
    return Plugin_Handled;
}

int WeaponOf(int client, const char[] cls)
{
    int n = GetEntPropArraySize(client, Prop_Send, "m_hMyWeapons");
    char have[32];
    for (int k = 0; k < n; k++) {
        int w = GetEntPropEnt(client, Prop_Send, "m_hMyWeapons", k);
        if (w > 0 && IsValidEdict(w)) {
            GetEdictClassname(w, have, sizeof(have));
            if (StrEqual(have, cls)) {
                return w;
            }
        }
    }
    return 0;
}

public Action OnPlayerRunCmd(int client, int &buttons, int &impulse, float vel[3], float angles[3],
    int &weapon, int &subtype, int &cmdnum, int &tickcount, int &seed, int mouse[2])
{
    if (client != g_bot || g_out == null) {
        return Plugin_Continue;
    }
    weapon = 0;
    impulse = 0;
    if (g_cursor < 0) {
        if (g_hasStart && IsPlayerAlive(client)) {
            SetEntityMoveType(client, MOVETYPE_WALK);
            TeleportEntity(client, g_startPos, g_startAng, g_startVel);
            angles[0] = g_startAng[0];
            angles[1] = g_startAng[1];
        }
        buttons = 0;
        vel[0] = 0.0;
        vel[1] = 0.0;
        vel[2] = 0.0;
        return Plugin_Changed;
    }
    if (g_cursor >= g_count) {
        return Plugin_Continue;
    }
    for (int k = 0; k < g_cmdCount; k++) {
        if (g_cmdRow[k] == g_cursor) {
            int c = g_cmdClient[k];
            if (c == 0) {
                ServerCommand("%s", g_cmdText[k]);
                ServerExecute();
            } else if (IsClientInGame(c)) {
                FakeClientCommand(c, "%s", g_cmdText[k]);
            }
            g_out.WriteLine("K %d %d %s", g_cursor, c, g_cmdText[k]);
        }
    }
    buttons = g_buttons[g_cursor];
    impulse = g_impulse[g_cursor];
    if (g_select[g_cursor][0] != '\0') {
        weapon = WeaponOf(client, g_select[g_cursor]);
    }
    vel[0] = g_forward[g_cursor];
    vel[1] = g_side[g_cursor];
    vel[2] = 0.0;
    angles[0] = g_pitch[g_cursor];
    angles[1] = g_yaw[g_cursor];
    angles[2] = 0.0;
    if (IsPlayerAlive(client)) {
        SetEntPropVector(client, Prop_Data, "v_angle", angles);
        TeleportEntity(client, NULL_VECTOR, angles, NULL_VECTOR);
    }
    return Plugin_Changed;
}

void WriteEntity(int e)
{
    char cls[64];
    GetEdictClassname(e, cls, sizeof(cls));
    float o[3];
    float v[3];
    GetEntPropVector(e, Prop_Data, "m_vecAbsOrigin", o);
    GetEntPropVector(e, Prop_Data, "m_vecAbsVelocity", v);
    float a[3];
    GetEntPropVector(e, Prop_Data, "m_angAbsRotation", a);
    int think = HasEntProp(e, Prop_Data, "m_nNextThinkTick") ? GetEntProp(e, Prop_Data, "m_nNextThinkTick") : -1;
    int effects = HasEntProp(e, Prop_Send, "m_fEffects") ? GetEntProp(e, Prop_Send, "m_fEffects") : -1;
    g_out.WriteLine("E %d %d %s %.4f %.4f %.4f %.4f %.4f %.4f %d %d %d %.3f %.3f %.3f", g_cursor, e, cls, o[0], o[1],
        o[2], v[0], v[1], v[2], think, GetGameTickCount(), effects, a[0], a[1], a[2]);
}

public void OnPlayerRunCmdPost(int client, int buttons, int impulse, const float vel[3], const float angles[3],
    int weapon, int subtype, int cmdnum, int tickcount, int seed, const int mouse[2])
{
    if (client != g_bot || g_out == null) {
        return;
    }
    if (g_cursor >= g_count) {
        delete g_out;
        g_out = null;
        PrintToServer("probe: done");
        return;
    }
    float o[3];
    float v[3];
    float maxs[3];
    float view[3];
    GetClientAbsOrigin(client, o);
    GetEntPropVector(client, Prop_Data, "m_vecAbsVelocity", v);
    GetEntPropVector(client, Prop_Send, "m_vecMaxs", maxs);
    GetEntPropVector(client, Prop_Data, "m_vecViewOffset", view);
    int flags = GetEntityFlags(client);
    int water = GetEntProp(client, Prop_Data, "m_nWaterLevel");
    float now = GetGameTime();
    g_cursor++;
    g_out.WriteLine("%d %.4f %.4f %.4f %.4f %.4f %.4f %d %d %.4f %.4f %d %d %d %.4f %d %d %.3f %d %d %.4f %d %d", g_cursor,
        o[0], o[1], o[2], v[0], v[1], v[2], (flags & FL_ONGROUND) ? 1 : 0, (flags & FL_DUCKING) ? 1 : 0, maxs[2],
        view[2], buttons, water, view_as<int>(GetEntityMoveType(client)), Aux(client), GetClientHealth(client),
        GetEntProp(client, Prop_Data, "m_ArmorValue"), GetEntPropFloat(client, Prop_Data, "m_flMaxspeed"),
        GetEntProp(client, Prop_Data, "m_lifeState"), GetEntProp(client, Prop_Send, "m_iObserverMode"), now,
        GetGameTickCount(), GetEntProp(client, Prop_Send, "m_nTickBase"));
    int w = GetEntPropEnt(client, Prop_Send, "m_hActiveWeapon");
    if (w > 0 && IsValidEdict(w)) {
        char cls[32];
        GetEdictClassname(w, cls, sizeof(cls));
        int type = GetEntProp(w, Prop_Send, "m_iPrimaryAmmoType");
        int type2 = GetEntProp(w, Prop_Send, "m_iSecondaryAmmoType");
        g_out.WriteLine("W %d %s %d %d %d %.4f %.4f", g_cursor, cls, GetEntProp(w, Prop_Send, "m_iClip1"),
            type >= 0 ? GetEntProp(client, Prop_Send, "m_iAmmo", _, type) : -1,
            type2 >= 0 ? GetEntProp(client, Prop_Send, "m_iAmmo", _, type2) : -1,
            GetEntPropFloat(w, Prop_Send, "m_flNextPrimaryAttack") - now,
            GetEntPropFloat(w, Prop_Send, "m_flNextSecondaryAttack") - now);
    }
    for (int k = 0; k < g_trackCount; k++) {
        if (IsValidEdict(g_track[k])) {
            WriteEntity(g_track[k]);
        }
    }
}

public void OnEntityCreated(int entity, const char[] classname)
{
    for (int k = 0; k < g_prefixCount; k++) {
        if (StrContains(classname, g_prefix[k]) == 0) {
            Track(entity);
            if (g_out != null) {
                g_out.WriteLine("C %d %d %s", g_cursor, entity, classname);
            }
            return;
        }
    }
}

public void OnEntityDestroyed(int entity)
{
    for (int k = 0; k < g_trackCount; k++) {
        if (g_track[k] == entity) {
            if (g_out != null) {
                char cls[64];
                GetEdictClassname(entity, cls, sizeof(cls));
                g_out.WriteLine("X %d %d %s", g_cursor, entity, cls);
            }
            g_track[k] = g_track[--g_trackCount];
            return;
        }
    }
}

public void DamagePost(int victim, int attacker, int inflictor, float damage, int damagetype)
{
    if (g_out != null) {
        int health = victim <= MaxClients ? GetClientHealth(victim)
                                          : (HasEntProp(victim, Prop_Data, "m_iHealth")
                                                 ? GetEntProp(victim, Prop_Data, "m_iHealth") : 0);
        g_out.WriteLine("D %d %d %d %d %.4f %d %d", g_cursor, victim, attacker, inflictor, damage, damagetype, health);
    }
}

public Action SoundHook(int clients[MAXPLAYERS], int &numClients, char sample[PLATFORM_MAX_PATH], int &entity,
    int &channel, float &volume, int &level, int &pitch, int &flags, char soundEntry[PLATFORM_MAX_PATH], int &seed)
{
    if (g_out != null && g_bot != 0) {
        if (entity == g_bot) {
            g_out.WriteLine("S %d %d %.3f %d %d %s %s", g_cursor, channel, volume, level, pitch, sample, soundEntry);
        } else if (entity > 0 && Tracked(entity)) {
            g_out.WriteLine("s %d %d %d %.3f %d %d %s %s", g_cursor, entity, channel, volume, level, pitch, sample,
                soundEntry);
        }
    }
    return Plugin_Continue;
}

public Action TE_Bullets(const char[] te_name, const int[] Players, int numClients, float delay)
{
    if (g_out != null) {
        float o[3];
        float d[3];
        TE_ReadVector("m_vecOrigin", o);
        TE_ReadVector("m_vecDir", d);
        g_out.WriteLine("T %d bullets %.3f %.3f %.3f %.5f %.5f %.5f %d %d %d %.5f %d", g_cursor, o[0], o[1], o[2], d[0],
            d[1], d[2], TE_ReadNum("m_iAmmoID"), TE_ReadNum("m_iSeed"), TE_ReadNum("m_iShots"),
            TE_ReadFloat("m_flSpread"), TE_ReadNum("m_iPlayer"));
    }
    return Plugin_Continue;
}

public Action TE_Effect(const char[] te_name, const int[] Players, int numClients, float delay)
{
    if (g_out != null) {
        char name[64] = "?";
        int table = FindStringTable("EffectDispatch");
        if (table != INVALID_STRING_TABLE) {
            ReadStringTable(table, TE_ReadNum("m_iEffectName"), name, sizeof(name));
        }
        g_out.WriteLine("T %d effect %s %.3f %.3f %.3f %.3f %.3f %.3f", g_cursor, name, TE_ReadFloat("m_vOrigin[0]"),
            TE_ReadFloat("m_vOrigin[1]"), TE_ReadFloat("m_vOrigin[2]"), TE_ReadFloat("m_vStart[0]"),
            TE_ReadFloat("m_vStart[1]"), TE_ReadFloat("m_vStart[2]"));
    }
    return Plugin_Continue;
}

public void Event_Hurt(Event e, const char[] name, bool dontBroadcast)
{
    int c = GetClientOfUserId(e.GetInt("userid"));
    if (g_out != null && c == g_bot) {
        g_out.WriteLine("H %d %d %d", g_cursor, GetClientHealth(c), GetEntProp(c, Prop_Data, "m_ArmorValue"));
    }
}

public void Event_Life(Event e, const char[] name, bool dontBroadcast)
{
    if (g_out != null) {
        int c = GetClientOfUserId(e.GetInt("userid"));
        if (StrEqual(name, "player_death")) {
            g_out.WriteLine("V %d %s %d %d", g_cursor, name, c, GetClientOfUserId(e.GetInt("attacker")));
        } else {
            g_out.WriteLine("V %d %s %d", g_cursor, name, c);
        }
    }
}

public void Event_Team(Event e, const char[] name, bool dontBroadcast)
{
    if (g_out != null) {
        g_out.WriteLine("V %d %s %d %d %d", g_cursor, name, GetClientOfUserId(e.GetInt("userid")), e.GetInt("team"),
            e.GetInt("oldteam"));
    }
}

public Action Command_Score(int args)
{
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i)) {
            PrintToServer("player %d team %d frags %d deaths %d life %d", i, GetClientTeam(i),
                GetEntProp(i, Prop_Data, "m_iFrags"), GetEntProp(i, Prop_Data, "m_iDeaths"),
                GetEntProp(i, Prop_Data, "m_lifeState"));
        }
    }
    int t = -1;
    while ((t = FindEntityByClassname(t, "team_manager")) != -1) {
        PrintToServer("team %d score %d", GetEntProp(t, Prop_Send, "m_iTeamNum"), GetEntProp(t, Prop_Send, "m_iScore"));
    }
    return Plugin_Handled;
}
