# Mobile Web SSH Simulator

This Linux-only simulator serves a phone-oriented Web UI over LAN and lets it
drive a Rust HTTP server that executes SSH commands against a disposable Linux
test host.

It is not iOS, macOS, Xcode, Windows, Keychain, local network permission, or
real runner evidence. It proves the Linux/LAN Web control path only.

## Start

Run from the repository root:

```bash
cargo run -p deepseek-mobile-web-server -- \
  --host 0.0.0.0 \
  --port 8788 \
  --ssh-host 192.168.30.244 \
  --ssh-user root \
  --ssh-port 22
```

For local-only testing, use `--host 127.0.0.1`.

The default development mode has no API authorization. For LAN testing where a
shared token is useful, the dev helper can pass it to the Rust server without
printing the raw value:

```bash
python3 scripts/mobile_web_dev.py \
  --print \
  --access-token "$MOBILE_WEB_TOKEN"
```

Printed helper commands show `--access-token REDACTED`. Evidence command logs
use `--access-token <redacted>`. Pass the real token value only through your
shell command line or environment. The server accepts the token through
`Authorization: Bearer <token>` or `X-Mobile-Web-Token: <token>`. `/health`
remains public.

The server prints the bind URL, LAN URL, SSH target, and model mode. It does not
print DeepSeek API tokens, SSH secrets, approval nonces, command leases, bearer
tokens, or idempotency keys.

## Web UI

The Web UI lives under `mobile-web/`. When Node/npm is available:

```bash
cd mobile-web
npm install
npm run typecheck
npm run build
```

The Rust server can serve a built Web bundle:

```bash
cargo run -p deepseek-mobile-web-server -- \
  --host 0.0.0.0 \
  --port 8788 \
  --static-dir mobile-web/dist
```

## SSH Behavior

Preset diagnostics execute directly over SSH:

```text
system_info: uname -a
current_user: id
disk_usage: df -h
memory: free -m || cat /proc/meminfo
network: ip addr || ifconfig
working_directory: pwd
```

Advanced commands always create a pending approval first. The first stage
supports `approve_once` and `reject`; it does not support `always approve`.

The production SSH runner invokes system `ssh` with `BatchMode=yes` and a short
connect timeout so validation does not hang on password prompts.

## Scripts

Smoke-check a running server:

```bash
python3 scripts/mobile_web_ssh_smoke.py \
  --server http://127.0.0.1:8788 \
  --json
```

Exercise the full HTTP approval flow:

```bash
python3 scripts/mobile_web_ssh_flow_simulator.py \
  --server http://127.0.0.1:8788 \
  --auto-approve \
  --json
```

When the server uses `--access-token dev-token`, add
`--access-token dev-token` to the smoke, flow, and evidence commands.

Run the script smoke test without a real SSH host:

```bash
python3 scripts/mobile_web_ssh_flow_simulator_smoke.py
```

Create a Linux/LAN Web simulator evidence folder for a live server:

```bash
python3 scripts/mobile_web_ssh_evidence.py \
  --server http://127.0.0.1:8788 \
  --host linux-lab-host \
  --auto-approve \
  --write-plan-draft
```

Final one-command Linux/Web/Rust/SSH control-chain verification, once the Rust
server is already running and pointed at the disposable Linux SSH host:

```bash
python3 scripts/mobile_web_ssh_evidence.py \
  --server http://127.0.0.1:8788 \
  --host linux-web-ssh-control-chain \
  --auto-approve \
  --write-plan-draft
```

If the server was started with `--access-token`, add
`--access-token "$MOBILE_WEB_TOKEN"` to the evidence command. The generated
bundle includes the smoke run, full approval-flow simulator run, captured
command logs, `results.md`, and `plan-update-draft.md`.

The helper writes a dated folder under `validation/mobile-web-ssh/` by default,
named like `YYYY-MM-DD-mobile-web-ssh-linux-lab-host/`. It creates `README.md`,
`evidence-log.md`, `environment.md`, `commands.log`, `results.md`, and tracked
`logs/`, `screenshots/`, and `failures/` placeholders. It then runs
`scripts/mobile_web_ssh_smoke.py` and
`scripts/mobile_web_ssh_flow_simulator.py` against `--server`, captures command
output, and exits nonzero if either command fails. With `--write-plan-draft`,
it also runs `scripts/mobile_evidence_plan_draft.py` against the just-created
bundle and writes `plan-update-draft.md` inside the evidence folder. The draft
is attempted after `results.md` is written, so failed smoke or flow commands are
still visible in both `commands.log` and the plan update draft.

Preview the planned paths and commands without writing files:

```bash
python3 scripts/mobile_web_ssh_evidence.py \
  --server http://127.0.0.1:8788 \
  --host linux-lab-host \
  --date 2026-05-25 \
  --write-plan-draft \
  --json \
  --dry-run
```

When `--write-plan-draft` is present, dry-run JSON includes the planned
`plan-update-draft.md` path without writing files.

This evidence helper is Linux/LAN Web simulator evidence only. It is not iOS,
macOS, Windows, or real mobile-platform evidence.

## SSH Check

`POST /api/ssh/check` runs the low-risk `true` command through the configured
SSH target and returns a structured reachability result. It does not require an
approval prompt. The Web UI exposes this as `Check SSH`, and
`scripts/mobile_web_ssh_smoke.py` calls the route before running preset
diagnostics.

## Evidence Boundary

Passing this simulator means:

- A LAN-accessible Rust server can expose phone-shaped HTTP/SSE APIs.
- Preset diagnostics can execute through SSH against a Linux host.
- Advanced commands are approval-gated before SSH execution.
- Audit output and script output are checked for token-like leaks.

It does not mean:

- Swift, Xcode, iOS simulator, or iOS device behavior has been validated.
- iOS Keychain, SQLite sandboxing, app lifecycle, or local network permission
  prompts work.
- macOS Homebrew/Xcode CLT or Windows PowerShell/UAC/PATH/Event Log behavior
  has been validated.
- Real browser automation, package installation, or production OS sandboxing is
  complete.
