#!/usr/bin/env python3
"""Assemble a Claude Code 'loaded' snapshot from on-disk settings.

Output: a single-line JSON object on stdout:
  {"plugins": [str, ...], "mcp_servers": [str, ...], "skills": [str, ...]}

Inputs (best-effort, missing files silently skipped):
  - ~/.claude/settings.json                                   enabledPlugins, mcpServers
  - $CWD/.claude/settings.json                                merged
  - $CWD/.claude/settings.local.json                          merged
  - $CWD/.mcp.json                                            mcpServers
  - $CWD/.claude/skills/*/SKILL.md                            project skills
  - ~/.claude/plugins/cache/**/skills/*/SKILL.md              plugin-provided skills
"""
import json, os, sys, glob

def _load(path):
    try:
        with open(os.path.expanduser(path), "r") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}

def main():
    cwd = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
    plugins, mcp, skills = set(), set(), set()

    for p in ["~/.claude/settings.json",
              os.path.join(cwd, ".claude/settings.json"),
              os.path.join(cwd, ".claude/settings.local.json")]:
        d = _load(p)
        for k, v in (d.get("enabledPlugins") or {}).items():
            if v: plugins.add(k)
        for k in (d.get("mcpServers") or {}).keys():
            mcp.add(k)

    for k in (_load(os.path.join(cwd, ".mcp.json")).get("mcpServers") or {}).keys():
        mcp.add(k)

    for skill_md in glob.glob(os.path.join(cwd, ".claude/skills/*/SKILL.md")):
        skills.add(os.path.basename(os.path.dirname(skill_md)))
    for skill_md in glob.glob(os.path.expanduser(
        "~/.claude/plugins/cache/**/skills/*/SKILL.md"), recursive=True):
        skills.add(os.path.basename(os.path.dirname(skill_md)))

    print(json.dumps({
        "plugins": sorted(plugins),
        "mcp_servers": sorted(mcp),
        "skills": sorted(skills),
    }, separators=(",", ":")))

if __name__ == "__main__":
    main()
