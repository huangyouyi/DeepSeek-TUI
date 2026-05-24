# Mobile Evidence Bundles

These templates define the empty evidence bundle structure for real-platform
mobile validation runs. They are skeletons only; they do not claim that macOS,
Windows, iOS simulator/device, or LAN runner validation has passed.

Create one dated bundle per run:

```text
validation/mobile/YYYY-MM-DD-<platform>-<host>/
```

Use `scripts/mobile_evidence_bundle.py` to inspect or generate a bundle:

```bash
python3 scripts/mobile_evidence_bundle.py --list
python3 scripts/mobile_evidence_bundle.py --platform macos-host --host macbook-pro --dry-run
python3 scripts/mobile_evidence_bundle.py --platform lan-runner --host lab-mac
```

Each generated bundle includes:

- `README.md`: bundle-specific path guide and redaction reminder.
- `evidence-log.md`: fillable result index for the platform checklist.
- `environment.md`: OS, toolchain, repo SHA, branch, and dirty-tree notes.
- `commands.log`: raw command transcript placeholder.
- `results.md`: pass/fail/blocker rollup and next action notes.
- `logs/`: raw logs, runner logs, device logs, `.xcresult` notes, or `.evtx`
  exports as appropriate.
- `screenshots/`: screenshots and screen recordings.
- `failures/`: crash reports, build reports, redacted network traces, and
  other failure-only artifacts.

Keep secrets out of evidence. Redact model API keys, bearer tokens, pairing
codes after redemption, SSH private keys, local usernames where needed, and
customer data.
