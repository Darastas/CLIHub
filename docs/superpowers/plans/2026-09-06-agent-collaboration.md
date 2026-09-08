# Agent Collaboration Implementation Plan

> Execute using superpowers:subagent-driven-development. Do not commit without user confirmation.

**Goal:** Add terminal-to-terminal handoffs, file-linked discussion, a development/review/fix workflow, and round-scoped Git inspection.

**Architecture:** Existing egui/PTY terminals stay interactive. A persistent local mailbox stores immutable message files and round metadata. A separate console executable exposes mailbox operations to any agent with shell access; no terminal-screen scraping is treated as an agent response. Review handoffs attach immutable Git snapshots. Automatic wake-up is not assumed: users explicitly paste a prepared instruction into an agent terminal, and agents can poll the mailbox with the helper.

**Tech Stack:** Rust, egui/eframe 0.35, serde_json, existing PTY, Git CLI. No web frontend or cloud service.

**Spec:** Approved conversation design: handoff + file discussions + development/review loop, original visual style and reference image.

## Global Constraints

- Work in D:\Terminal desk; retain AGENTS.md and reference image.
- Chinese user-facing text and future commit descriptions.
- Build target/debug and target/release. No auto commit or push.
- Do not infer per-agent authorship from shared working-tree changes.
- Store collaboration data outside source repository by default.
- Use explicit sender/recipient IDs, reply IDs and snapshot references.
- Draft instructions require an explicit paste command; never auto-submit to arbitrary PTY.

## Tasks

- [ ] 1. Mailbox: create src/collab/{mod,store}.rs and src/bin/clihub-agent.rs. Persistent Round, Message, Participant, phase enum. Atomic per-message writes; read acknowledgement separate from messages; CLI list/read/send/reply. Validate recipients and reply round membership. Test two agents exchanging replies, persistence and invalid recipients.
- [ ] 2. Git: create src/collab/git.rs. GitSnapshot captures HEAD, branch, porcelain status, staged/unstaged patch, untracked text preview and changed-file metadata. Compare file fingerprints to a round baseline, handle initial repositories and deleted/renamed files. Git commands run off UI thread. Tests in disposable repositories for baseline changes and staged/unstaged files.
- [ ] 3. UI/integration: add src/ui/collaboration.rs and src/app/collaboration.rs, wire module declarations. Add stable tab IDs. Add sidebar Agent list and collaboration toggle; resizable dual-terminal workspace, right inspector with messages/Git/files and workflow controls. Existing terminal show remains wrapper; explicit-tab rendering uses distinct egui IDs and input focus. Room/participants persist; process binding is runtime-only. File discussion references snapshot/file/line. Helper invocation instructions are available for copy or explicit paste without Enter.
- [ ] 4. Verification/docs: cargo test, cargo build, cargo build --release. Exercise helper process A-to-B-to-A and Git snapshot stability. Review full diff and fix defects; inspect native app when available, clearly report unavailable visual checks. Update README.zh-CN.md with helper commands, mailbox semantics and UI workflow. Ask about a local Chinese commit only after work completes.

## Interface Contracts

Mailbox module APIs are supplied by its implementer and consumed by UI after reading the finished module; avoid shared file ownership. Git snapshot module is independent of store; UI serializes snapshots as separate files and references their IDs in messages. Both use anyhow::Result and serde types.

## Progress / Rulings

- 2026-09-07 startup failure investigation: screenshot only exposed the outer anyhow context. Startup errors now retain the complete error chain, executable and cwd, display in the terminal, and persist to the workflow's last-start-error.txt. Fixed splitting existing executable paths containing spaces; real ConPTY regression passed. Installed Codex 0.153.4 started successfully in the user's workflow cwd, including from a temporary Windows-subsystem probe. The reported GUI failure was not reproduced; do not claim its root cause is confirmed. Normal suite: 26 unit and 2 integration tests passed. Removed temporary probe source after diagnosis.

- 2026-09-07 native terminal implementation: Agent Chat now uses the existing terminal renderer and ConPTY process, with one column per selected participant and a collapsible conversation below. Roles moved beside the workflow picker; participant sidebar removed. Icon controls reuse sidebar background, border, shadow and hover values. Multi-terminal input requires focus, and dropped files target only the hovered embedded terminal.
- Native execution uses per-request task files and existing mailbox read/send commands. An explicit read receipt marks task acceptance; a reply linked to the active request drives automatic handoff. Cancellation closes the native process; submitted terminal output remains visible until that role runs again. Restart preserves messages, but terminal buffers and running sessions are not restored.
- Live Codex native test passed: developer received task, submitted result, reviewer automatically launched, received task, submitted result, workflow completed in 284.62 seconds. No manual second send. Earlier 150-second test observation expired; this was a test limit only, not a product timeout. Native Claude verification pending.

- 2026-09-07 latest ruling: one submitted task starts automatic role cycling; handoff executes immediately. There is no turn cap or per-process timeout. After a full team cycle, an explicit final completion marker from the last participant completes the workflow; otherwise execution continues. User cancellation and actual errors stop execution without claiming completion. This supersedes earlier manual-only and timeout notes below.
- Live automatic flow verified: ONE execute call -> developer Codex -> reviewer Codex -> completed, with persisted request/reply links. Claude's previously documented task-following failure remains unresolved.

- 2026-09-07: user approved real execution integration. Send now starts an isolated noninteractive CLI worker using saved member command and round cwd; supplies role, goal and last 20 messages; persists actual answers with reply linkage. Cancellation and timeout supported; workflow switch does not redirect output.
- Verified real application state flow: developer Codex response handed to separate reviewer Codex invocation and response persisted. Cancel probe passed. Normal tests: 26 passed, 7 ignored.
- Claude adapter implemented, but live exact-answer probe failed: stdin, argument and stream-json input all yielded generic greeting. Do not claim Claude business verification passed. No credentials/configuration changed.
- Remaining: native screenshot/input verification, live output streaming, automatic multi-turn orchestration and Git inspector.

- 2026-09-06 scope correction approved by user: prioritize bottom-left Agent Chat, workflow creation from Workspaces with per-AI roles, multiple persistent workflows and usable message composer. Dual terminals and automatic scheduling remain unimplemented.
- Complete: basic mailbox and console helper; unique atomic message publication; user-to-agent and agent-to-agent send/reply tested with independent processes.
- Complete: workflow creation, selected AI roles, separate history, phase persistence, all-participant messages, recipient selection and replies. Startup accepts --agent-chat.
- Complete: layout boundary checks at 760x480, 1120x720 and 1600x900 using application fonts. These are headless egui tests, not native screenshot review.
- Pending from original scope: Git inspector UI and baseline integration, file-thread creation UI, dual-terminal view, agent auto-launch/wake-up. Existing Git backend remains separate and is not presented as an operational feature of this iteration.

- Plan reviewed: mailbox and Git modules share no files; UI consumes both after interfaces are delivered. Integration owns all existing files and Cargo.toml.
- Ruling: keep the user-requested checkout and create a feature branch; do not create another checkout or commit plan automatically.
- Ruling: mailbox CLI is the first real agent transport. MCP/native wake-up adapters are future work; do not claim these work in this build.
