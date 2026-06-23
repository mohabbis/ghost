# Command Registry

Ghost's Tauri command names are frontend IPC compatibility contracts. Keep the public names registered in `src-tauri/src/lib.rs` stable unless the app deliberately ships an IPC breaking change.

Command implementations live in `src-tauri/src/commands/`:

| Module | Boundary | Commands |
| --- | --- | --- |
| `core.rs` | Stable recording, replay, workflow storage, inspection, permissions, and Ghost Guard audit. | `start_recording`, `stop_recording`, `replay_workflow`, `ghost_guard_audit`, replay controls, inspection commands, workflow CRUD, permission checks |
| `auth.rs` | Local auth and at-rest workflow protection. | `auth_status`, `auth_setup`, `auth_unlock`, `auth_lock` |
| `diagnostics.rs` | Configuration, telemetry, performance, execution history, and analytics. | `get_config`, `update_config`, telemetry export, performance summary, execution history, workflow analytics |
| `experimental.rs` | AI, cloud/workspace sync, visual checks, data-driven testing, reliability experiments, and observer mode. | AI workflow commands, metadata/sidecar workflow commands, cloud commands, visual/data/reliability commands, observer and learned-pattern commands |

`commands/mod.rs` re-exports every grouped command so the registration path remains `commands::command_name`. New commands should be added to the appropriate module first, then registered in `lib.rs` under the matching group.
