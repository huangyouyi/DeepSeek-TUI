# Mobile Evidence Plan Draft Parser

`scripts/mobile_evidence_plan_draft.py` parses `Result summary` tables from
mobile validation evidence logs and prints a markdown draft for updating
`docs/mobile-porting-plan.md`. It does not edit the plan automatically.

Inputs can be:

- `docs/mobile-validation-checklists.md`, to produce a blank scaffold from the
  checklist templates.
- A single `evidence-log.md` file.
- An evidence bundle directory such as
  `validation/mobile/2026-05-24-ios-sim/`; the script searches below the
  directory for `evidence-log.md` and `results.md` files.
- A Linux/LAN mobile Web SSH simulator bundle from
  `scripts/mobile_web_ssh_evidence.py`, which contains `evidence-log.md`,
  `results.md`, and `commands.log`.

The generated draft contains these fields:

| Field | Source |
|---|---|
| Platform | Evidence log heading, checklist section heading, or bundle path |
| Pass/Fail/Blocker | `Pass/Fail/Blocker` result summary column |
| Milestone | `Related milestone` result summary column |
| Next task | `Area` plus `Next issue/task` result summary columns |
| Plan updated | `` `mobile-porting-plan.md` updated? `` result summary column |
| Source | Source markdown file that provided the row |

If no result summary rows are found, the parser exits with an error that names
the evidence files it checked.

Mobile Web SSH simulator bundles do not use the generic result-summary table.
For those bundles, the parser reads the command PASS/FAIL lines from
`results.md`, requires sibling `evidence-log.md` and `commands.log`, and emits a
Linux/LAN Web SSH Simulator section. That section preserves the evidence
boundary exactly: it is Linux/LAN Web simulator evidence only, not iOS, macOS,
Windows, or real mobile-platform/runner evidence.

## Linux Smoke Commands

Run the parser against the checklist templates:

```bash
python3 scripts/mobile_evidence_plan_draft.py docs/mobile-validation-checklists.md
```

Run the smoke test:

```bash
python3 scripts/mobile_evidence_plan_draft_test.py
```

Parse an evidence bundle and keep source file paths in the draft:

```bash
python3 scripts/mobile_evidence_plan_draft.py validation/mobile/2026-05-24-ios-sim/
```

Parse a Linux/LAN mobile Web SSH simulator evidence bundle:

```bash
python3 scripts/mobile_evidence_plan_draft.py validation/mobile-web-ssh/2026-05-25-mobile-web-ssh-linux-lab-host/
```

`--source` is still accepted for compatibility, but the Source column is now
included by default.

## LAN Runner Evidence Prep

`scripts/mobile_lan_runner_evidence.py` writes passive LAN runner evidence
bundles. Its generated `commands.log` and appended `results.md` notes include
the Linux iOS simulator commands that should be captured by the LF-E evidence
loop once `scripts/mobile_ios_flow_simulator.py` is available:

```bash
python3 scripts/mobile_ios_flow_simulator.py --dry-run
python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json
```
