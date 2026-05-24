# iOS Device Evidence Bundle

Use this bundle for the physical iOS device checklist in
`docs/mobile-validation-checklists.md`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: device model, iOS version, signing profile/team notes,
  Xcode version, repo SHA, branch, and dirty-tree status.
- `commands.log`: Xcode build/install, device log, and validation transcript.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: device console logs, crash reports, provisioning/signing reports,
  app logs, and runner access logs for LAN checks.
- `screenshots/`: physical-device screenshots and recordings.
- `failures/`: signing failures, lifecycle failures, crash reports, and
  redacted network traces.

Do not store production secrets or customer data in this bundle.
