# Agent Eval

Local `it-runner` project for comparing kai, OpenCode, and CodeWhale.

## Environment

- it-runner API: `http://127.0.0.1:35319`
- kai API: `http://127.0.0.1:8788`
- remote runner host: `root@192.168.30.244`
- remote root: `/mnt/vio3-1/kai-test`
- execution model: `deepseek-v4-flash`
- judge model: `deepseek-v4-pro`
- kai auto approval: `KAI_AUTO_APPROVE=1`, sent as `auto_approve: true` to the kai mobile API

## Secret Setup

Create `.it-runner/envs/080-secret-local.env`:

```env
SECRET_DEEPSEEK_API_KEY=...
```

This file is ignored by git.

## Smoke Checks

Verify kai:

```bash
curl -s http://127.0.0.1:8788/health
```

Verify remote OpenCode:

```bash
ssh root@192.168.30.244 'cd /mnt/vio3-1/kai-test && ./bin/run-opencode "只回复 OK"'
```

Verify remote CodeWhale:

```bash
ssh root@192.168.30.244 'cd /mnt/vio3-1/kai-test && ./bin/run-codewhale "只回复 OK"'
```

## Manual One-Case Run

```bash
cd agent-eval
set -a
source .it-runner/envs/000-defaults.env
source .it-runner/envs/080-secret-local.env
set +a
export PROJECT_ROOT="$PWD"
export EVAL_ROOT="$PWD"
export EVAL_RUNS_DIR="$PWD/runs"
export EVAL_REPORTS_DIR="$PWD/reports"
export EVAL_CASES_DIR="$PWD/cases"
export EVAL_SCRIPTS_DIR="$PWD/scripts"
export EVAL_RUN_ID="manual-smoke"
export PLATFORM="kai"
export CASE_ID="case整理资料"
export REPEAT_INDEX="1"
python3 scripts/run_case.py
python3 scripts/normalize_result.py
python3 scripts/judge_result.py
```

## Build Report

```bash
export EVAL_RUN_ID="manual-smoke"
python3 scripts/build_report.py
```
