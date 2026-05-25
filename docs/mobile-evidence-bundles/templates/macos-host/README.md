# macOS Host Evidence Bundle

Use this bundle for the macOS host checklist in
`docs/mobile-validation-checklists.md`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: `sw_vers`, `uname -m`, Xcode, Swift, simulator runtime,
  repo SHA, branch, and dirty-tree status.
- `commands.log`: terminal transcript for every command/action.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: Xcode logs, `.xcresult` references, Swift build/test output,
  simulator logs, and runner bootstrap logs.
- `screenshots/`: Xcode scheme/destination, simulator UI, and failure images.
- `failures/`: crash reports, compiler/linker diagnostics, install failures,
  and redacted network traces.

Do not store production secrets or customer data in this bundle.
