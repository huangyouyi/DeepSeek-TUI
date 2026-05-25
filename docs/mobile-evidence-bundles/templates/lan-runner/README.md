# LAN Runner Evidence Bundle

Use this bundle for the LAN runner checklist in
`docs/mobile-validation-checklists.md`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: runner host OS, runner version/artifact, bind address,
  firewall profile, repo SHA, branch, and dirty-tree status if a checkout
  exists.
- `commands.log`: runner startup, curl/app calls, pairing, auth rejection, and
  interruption transcript.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: runner stdout/stderr, service manager logs, access logs, app logs,
  and firewall/router notes.
- `screenshots/`: phone/simulator connection state, pairing UX, and network
  prompts.
- `failures/`: service failures, HTTP transcripts, crash reports, and redacted
  network traces.

Do not store production secrets or customer data in this bundle.
