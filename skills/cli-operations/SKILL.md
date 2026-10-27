---
name: cli-operations
description: Operate Coaxi's built-in browser, right-side browser panel and shared desktop windows through Coaxi tools; diagnose missing capabilities and choose the matching MCP or CLI. Also coordinate messages, Tasks, Missions, automation, history, Memory and member requests. For one known operation, use its command help directly.
---

# Coaxi CLI coordination

Use `coaxi --help` to find an operation and its exact `--help` for syntax. Read only the references needed by the current decision. Write user-facing prose in the user's language.

## Coaxi built-in resources

When the user names Coaxi's built-in browser, its right-side browser panel, or a page shared in Coaxi, use the tools of the coaxi-interaction MCP server for that resource. Start with browser_list_tabs, then browser_snapshot for the chosen tab. Use the returned tab identity, generation and element ref for later actions. Do not substitute Chrome, another browser, a desktop screenshot, an external computer-use skill, or a web search for this named target.

When the user names a desktop window shared in Coaxi, use computer_list_windows and computer_snapshot from coaxi-interaction for that window. A visible window or tab is not a grant. Reading needs a read grant; clicking, typing and other writes need a control grant. browser_eval needs its own enabled capability and user approval for the exact code.

For Coaxi tool availability, use coaxi_diagnose_capabilities when present. Distinguish an assigned capability, its frozen delivery to the current run, a successful native tool call, and a resource grant. Do not claim one from evidence of another. If the MCP tools are absent or discovery fails, describe the missing Coaxi connection and the smallest recovery action. Do not invent a coaxi browser/computer CLI command, enable a capability, widen a grant, or switch to another resource on the user's behalf. After a write returns an unknown outcome, inspect current state before any retry.

## Choose the result to preserve

| Need | Operation family |
| --- | --- |
| Public answer, progress, question or one-time collaboration | ThreadMessage |
| Shared objective or whole-Mission status | Mission |
| Member identity, profile images or a requested profile edit | Member |
| Default Lead requests independent work from several members | One ThreadMessage with repeated `--to`; replies return separately |
| Responsibility that survives Runs and can be handed off and accepted independently | Task |
| Thread or message evidence | Thread/History |
| Who is running, queued or waiting | Thread execution query |
| Durable collaboration preference, agreement or lesson | Memory governance |

Choose the smallest object that fully serves the request. Tasks own durable responsibilities; project sources and history own their facts.

## Member operations

`member get` supplies the current Profile version and image paths; `member update` changes the global Profile. Send only the fields the User asked to change. For a newly created member, use the `agentId` returned by `member create`, even before it joins this Thread. Use `member-studio` for creation proposals and confirmation, not edits to an existing Profile.

## Coordinate operations

1. Read the authoritative state needed for the decision.
2. Inspect the committed business result before taking the next step.
3. Publish any required Thread-visible answer before ending the Run.

A successful operation proves its own commit, not downstream execution, validation or completion of the user's objective.

## References

- [Send](references/send.md): public messages, Agent routing, parallel invitations and User attention.
- [Task](references/task.md): durable responsibility and Task-linked messages.
- [Mission](references/mission.md): objective, status and public explanation.
- [Thread/History](references/camp-history.md): message reads, addressing, execution state and pagination.
- [Memory](references/memory.md): route durable information to `memory-stewardship`.
- [Recovery](references/recovery.md): follow `error.recovery`, especially uncertain outcomes.
