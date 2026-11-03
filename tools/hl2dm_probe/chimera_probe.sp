// chimera_probe (HL2:DM): replays per-tick inputs on a bot in a real
// Half-Life 2: Deathmatch dedicated server and logs its state after every
// tick, so Chimera's HL2:DM movement and suit can be compared with the
// game tick for tick (tools/hl2dm_probe/README.md; the CS:S probe's
// format, tools/css_probe/mashup_probe.sp, plus the suit's numbers).
//
// Server commands:
//   probe_run <input file> <output file>   (paths relative to hl2mp/):
//     runs on the first living fake client (an `hl2mp_bot_add` bot; its
//     commands are replaced every tick, so its AI moves nothing).
//     Input: one optional "start x y z pitch yaw vx vy vz" line, then one
//     line per tick "buttons forwardmove sidemove pitch yaw".
//     Output: "tick x y z vx vy vz onground ducked hull_top eye buttons
//     water movetype aux health armor maxspeed" per tick (tick 0: placed),
//     "S tick channel volume level pitch sample" for the bot's sounds and
//     "H tick health armor damage type" for its hurt events.
//   probe_set <health> <armor> <aux>: the bot's health, armour, aux power.
//   probe_info: every living player's position and angles.

#include <sourcemod>
#include <sdktools>

public Plugin myinfo = {
    name = "chimera probe (HL2:DM)",
    author = "chimera",
    description = "Replays per-tick inputs on a bot and logs its movement and suit",
    version = "1",
};

#define MAX_TICKS 16384

int g_buttons[MAX_TICKS];
float g_forward[MAX_TICKS];
float g_side[MAX_TICKS];
float g_pitch[MAX_TICKS];
float g_yaw[MAX_TICKS];
int g_count;
float g_startPos[3];
float g_startAng[3];
float g_startVel[3];
bool g_hasStart;
int g_bot;
File g_out;
int g_cursor = -1;

public void OnPluginStart()
{
    AddNormalSoundHook(SoundHook);
    HookEvent("player_hurt", Event_Hurt);
    RegServerCmd("probe_run", Command_Run, "probe_run <input> <output>");
    RegServerCmd("probe_set", Command_Set, "probe_set <health> <armor> <aux>");
    RegServerCmd("probe_info", Command_Info, "probe_info: living players' positions and angles");
}

int FindBot()
{
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i) && IsFakeClient(i) && IsPlayerAlive(i)) {
            return i;
        }
    }
    return 0;
}

float Aux(int client)
{
    if (HasEntProp(client, Prop_Send, "m_flSuitPower")) {
        return GetEntPropFloat(client, Prop_Send, "m_flSuitPower");
    }
    return -1.0;
}

public Action Command_Set(int args)
{
    int bot = FindBot();
    if (bot == 0) {
        PrintToServer("probe: no living bot");
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

public Action Command_Info(int args)
{
    for (int i = 1; i <= MaxClients; i++) {
        if (IsClientInGame(i) && IsPlayerAlive(i)) {
            float o[3];
            float a[3];
            GetClientAbsOrigin(i, o);
            GetClientEyeAngles(i, a);
            PrintToServer("player %d fake %d at %.2f %.2f %.2f angles %.2f %.2f aux %.3f", i, IsFakeClient(i), o[0], o[1],
                o[2], a[0], a[1], Aux(i));
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
    g_hasStart = false;
    char line[256];
    char parts[10][32];
    while (f.ReadLine(line, sizeof(line))) {
        TrimString(line);
        if (line[0] == '\0') {
            continue;
        }
        int n = ExplodeString(line, " ", parts, sizeof(parts), sizeof(parts[]));
        if (StrEqual(parts[0], "start") && n >= 9) {
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
            g_count++;
        }
    }
    delete f;
    g_bot = FindBot();
    if (g_bot == 0) {
        PrintToServer("probe: no living bot");
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

public Action OnPlayerRunCmd(int client, int &buttons, int &impulse, float vel[3], float angles[3],
    int &weapon, int &subtype, int &cmdnum, int &tickcount, int &seed, int mouse[2])
{
    if (client != g_bot || g_out == null) {
        return Plugin_Continue;
    }
    weapon = 0;
    impulse = 0;
    if (g_cursor < 0) {
        if (g_hasStart) {
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
    buttons = g_buttons[g_cursor];
    vel[0] = g_forward[g_cursor];
    vel[1] = g_side[g_cursor];
    vel[2] = 0.0;
    angles[0] = g_pitch[g_cursor];
    angles[1] = g_yaw[g_cursor];
    angles[2] = 0.0;
    SetEntPropVector(client, Prop_Data, "v_angle", angles);
    TeleportEntity(client, NULL_VECTOR, angles, NULL_VECTOR);
    return Plugin_Changed;
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
    g_cursor++;
    g_out.WriteLine("%d %.4f %.4f %.4f %.4f %.4f %.4f %d %d %.4f %.4f %d %d %d %.4f %d %d %.3f", g_cursor, o[0], o[1],
        o[2], v[0], v[1], v[2], (flags & FL_ONGROUND) ? 1 : 0, (flags & FL_DUCKING) ? 1 : 0, maxs[2], view[2], buttons,
        water, view_as<int>(GetEntityMoveType(client)), Aux(client), GetClientHealth(client),
        GetEntProp(client, Prop_Data, "m_ArmorValue"), GetEntPropFloat(client, Prop_Data, "m_flMaxspeed"));
}

public Action SoundHook(int clients[MAXPLAYERS], int &numClients, char sample[PLATFORM_MAX_PATH], int &entity,
    int &channel, float &volume, int &level, int &pitch, int &flags, char soundEntry[PLATFORM_MAX_PATH], int &seed)
{
    if (g_out != null && g_bot != 0 && entity == g_bot) {
        g_out.WriteLine("S %d %d %.3f %d %d %s %s", g_cursor, channel, volume, level, pitch, sample, soundEntry);
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
