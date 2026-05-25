# Windows Host Evidence Bundle

Use this bundle for the Windows host checklist in
`docs/mobile-validation-checklists.md`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: Windows version, architecture, PowerShell, privilege,
  execution policy, repo/artifact context, and dirty-tree status if a checkout
  exists.
- `commands.log`: PowerShell transcript and command output.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: runner logs, installer logs, Event Viewer exports, and HTTP
  transcripts with tokens redacted.
- `screenshots/`: UAC prompts, setup failures, and policy dialogs.
- `failures/`: `.evtx` exports, crash reports, install reports, and redacted
  network traces.

Do not store production secrets or customer data in this bundle.
