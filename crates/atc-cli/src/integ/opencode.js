// Written by `atc hook opencode`. Rewritten by `atc hook -u`, removed by `atc unhook opencode`.
// The whole file is tower's: a byte changed reads as stale, and `atc hook -u` rewrites it.
const ATC = __ATC__;

export const TowerPlugin = async ({ $, directory }) => ({
  // The one variable is both the session tag and the client marker.
  "shell.env": async (input, output) => {
    if (input.sessionID) output.env.OPENCODE_SESSION_ID = input.sessionID;
  },
  // The notice, standing: in the system prompt of every model call, so it
  // says what the board says now and survives compaction. A trigger that
  // fails or says nothing puts nothing in the prompt.
  "experimental.chat.system.transform": async (input, output) => {
    const payload = JSON.stringify({ hook_event_name: "SessionStart", session_id: input.sessionID ?? "", cwd: directory });
    const text = await $`echo ${payload} | ${ATC} trigger opencode`.cwd(directory).nothrow().text();
    if (text.trim()) output.system.push(text.trim());
  },
  // The heartbeat: the lease renewed on every tool call, nothing printed.
  "tool.execute.before": async (input) => {
    const payload = JSON.stringify({ hook_event_name: "PreToolUse", session_id: input.sessionID, cwd: directory });
    await $`echo ${payload} | ${ATC} trigger opencode`.cwd(directory).nothrow().quiet();
  },
});
