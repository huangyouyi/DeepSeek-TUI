# iOS Simulator Evidence Bundle

Use this bundle for the iOS simulator checklist in
`docs/mobile-validation-checklists.md`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: macOS/Xcode/simulator runtime, selected device UDID, repo
  SHA, branch, and dirty-tree status.
- `commands.log`: `simctl`, `xcodebuild`, install, launch, and navigation
  transcript.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: `.xcresult` references, simulator logs, app logs, crash reports, and
  runner access logs for local-network rehearsal.
- `screenshots/`: simulator screens and recordings.
- `failures/`: build reports, linker diagnostics, crash reports, and redacted
  network traces.

Do not store production secrets or customer data in this bundle.
