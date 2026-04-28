# AI Focus Command Center Plan

## Summary
Turn the app into a single-user desktop command center for leadership focus across repos, GitHub, TFS/ADO, Confluence, and personal meeting notes. The first serious version should center on app-owned projects, a hybrid leadership kanban, explainable prioritization, and read-only AI advisor agents.

## Key changes
- Add first-class projects that link repos and external source bindings.
- Add editable agent profiles in the UI with prompts, source scopes, and scoring weights.
- Make the landing page a command center with priorities and a leadership kanban.
- Prepare the app to manage `../meeting-tool/mcp-server` as a future read-only source.

## Acceptance
- A user can define active projects in the app without relying on repo structure.
- The home screen answers what needs attention now.
- Agent profiles are visible and editable from the UI.
- The app stays local-first and read-only for external systems.
