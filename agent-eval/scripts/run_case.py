#!/usr/bin/env python3
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from shlex import quote


def env(name: str, default: str | None = None) -> str:
    value = os.environ.get(name, default)
    if value is None or value == "":
        raise SystemExit(f"missing required env: {name}")
    return value


def run(cmd: list[str], *, input_bytes: bytes | None = None, timeout: int = 900) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )


def remote_shell(remote_host: str, command: str, *, timeout: int = 900) -> subprocess.CompletedProcess:
    return run(["ssh", remote_host, command], timeout=timeout)


def post_json(url: str, payload: dict, timeout: int = 300) -> dict:
    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def copy_case_to_remote(case_dir: Path, remote_host: str, remote_workspace: str) -> None:
    remote_shell(remote_host, f"rm -rf {quote(remote_workspace)} && mkdir -p {quote(remote_workspace)}")
    source = case_dir / "input"
    if not source.exists():
        source = case_dir
    tar = subprocess.Popen(
        ["tar", "-C", str(source), "-cf", "-", "."],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    ssh = subprocess.run(
        ["ssh", remote_host, f"tar -C {quote(remote_workspace)} -xf -"],
        stdin=tar.stdout,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    _, tar_stderr = tar.communicate()
    if tar.returncode != 0:
        raise RuntimeError(f"local tar failed: {tar_stderr.decode('utf-8', 'replace')}")
    if ssh.returncode != 0:
        raise RuntimeError(f"remote tar failed: {ssh.stderr.decode('utf-8', 'replace')}")


def copy_remote_workspace(remote_host: str, remote_workspace: str, local_workspace: Path) -> None:
    shutil.rmtree(local_workspace, ignore_errors=True)
    local_workspace.mkdir(parents=True, exist_ok=True)
    ssh = subprocess.Popen(
        ["ssh", remote_host, f"tar -C {quote(remote_workspace)} -cf - ."],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    tar = subprocess.run(
        ["tar", "-C", str(local_workspace), "-xf", "-"],
        stdin=ssh.stdout,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    _, ssh_stderr = ssh.communicate()
    if ssh.returncode != 0:
        raise RuntimeError(f"remote workspace export failed: {ssh_stderr.decode('utf-8', 'replace')}")
    if tar.returncode != 0:
        raise RuntimeError(f"local workspace import failed: {tar.stderr.decode('utf-8', 'replace')}")


def platform_command(platform: str, remote_root: str, remote_workspace: str, prompt: str) -> tuple[str, str]:
    if platform == "opencode":
        cmd = (
            f"cd {quote(remote_root)} && "
            f"EVAL_WORKDIR={quote(remote_workspace)} ./bin/run-opencode {quote(prompt)}"
        )
        return cmd, ""
    if platform == "codewhale":
        container_workspace = remote_workspace.replace(remote_root.rstrip("/"), "/kai-test", 1)
        cmd = (
            f"cd {quote(remote_root)} && "
            f"EVAL_WORKDIR={quote(container_workspace)} ./bin/run-codewhale {quote(prompt)}"
        )
        return cmd, ""
    raise ValueError(f"unsupported remote platform: {platform}")


def extract_opencode_session(stdout: str) -> str | None:
    match = re.search(r'"sessionID"\s*:\s*"([^"]+)"', stdout)
    return match.group(1) if match else None


def run_kai(kai_server: str, prompt: str) -> tuple[str, str, int, dict]:
    session = post_json(f"{kai_server.rstrip('/')}/api/sessions", {"title": "agent eval"})
    session_id = session["id"]
    auto_approve = os.environ.get("KAI_AUTO_APPROVE", "1") not in {"0", "false", "False"}
    max_rounds = int(os.environ.get("KAI_AGENT_MAX_TURNS", os.environ.get("KAI_AUTO_APPROVE_MAX_ROUNDS", "8")))
    turns = []
    approvals = []
    message = prompt
    turn = {}
    for round_index in range(max_rounds):
        turn = post_json(
            f"{kai_server.rstrip('/')}/api/sessions/{session_id}/agent-turn",
            {"message": message, "model_mode": "deepseek", "auto_approve": auto_approve},
            timeout=900,
        )
        turns.append(turn)
        pending = turn.get("pending_approvals") if isinstance(turn, dict) else []
        answer = str(turn.get("assistant_answer") or "")
        if not pending and ("整理完成" in answer or "FINAL_DONE" in answer or "完成" in answer and "README" in answer):
            break
        if pending and not auto_approve:
            break
        for approval in pending:
            approval_id = approval.get("id")
            if not approval_id:
                continue
            response = post_json(
                f"{kai_server.rstrip('/')}/api/approvals/{approval_id}/respond",
                {"response": "approve_session"},
                timeout=900,
            )
            approvals.append(response)
        message = (
            "请继续完成原始任务，仍然只在指定工作目录内操作。"
            "不要只说明计划，请直接执行下一步。"
            "如果已经生成 README.md 和行动清单并完成整理，请在总结中写 FINAL_DONE。"
        )
    messages = urllib.request.urlopen(
        f"{kai_server.rstrip('/')}/api/sessions/{session_id}/messages",
        timeout=120,
    ).read().decode("utf-8")
    body = {
        "session_id": session_id,
        "turn": turn,
        "turns": turns,
        "auto_approval_responses": approvals,
        "messages": json.loads(messages),
    }
    return json.dumps(body, ensure_ascii=False, indent=2), "", 0, body


def main() -> int:
    root = Path(env("EVAL_ROOT"))
    cases_dir = Path(env("EVAL_CASES_DIR"))
    runs_dir = Path(env("EVAL_RUNS_DIR"))
    platform = env("PLATFORM")
    case_id = env("CASE_ID")
    repeat = env("REPEAT_INDEX")
    run_id = env("EVAL_RUN_ID")
    remote_host = env("REMOTE_HOST")
    remote_root = env("REMOTE_ROOT")
    kai_server = env("KAI_SERVER", "http://127.0.0.1:8788")

    case_dir = cases_dir / case_id
    prompt_path = case_dir / "prompt.txt"
    if not prompt_path.exists():
        raise SystemExit(f"missing case prompt: {prompt_path}")

    out_dir = runs_dir / run_id / platform / case_id / repeat
    raw_dir = out_dir / "raw"
    workspace_dir = out_dir / "workspace"
    raw_dir.mkdir(parents=True, exist_ok=True)

    remote_workspace = f"{remote_root.rstrip('/')}/eval-runs/{run_id}/{platform}/{case_id}/{repeat}/workspace"
    copy_case_to_remote(case_dir, remote_host, remote_workspace)

    base_prompt = prompt_path.read_text(encoding="utf-8")
    prompt = (
        f"{base_prompt}\n\n"
        f"请只在这个工作目录内操作：{remote_workspace}\n"
        "完成后请给出简短总结，并列出你创建、修改或移动的文件。"
    )

    started = time.time()
    stdout = ""
    stderr = ""
    returncode = 1
    extra: dict = {}
    try:
        if platform == "kai":
            stdout, stderr, returncode, extra = run_kai(kai_server, prompt)
        else:
            cmd, _ = platform_command(platform, remote_root, remote_workspace, prompt)
            completed = remote_shell(remote_host, cmd, timeout=900)
            stdout = completed.stdout.decode("utf-8", "replace")
            stderr = completed.stderr.decode("utf-8", "replace")
            returncode = completed.returncode
            if platform == "opencode":
                session_id = extract_opencode_session(stdout)
                if session_id:
                    export_cmd = f"cd {quote(remote_root)} && HOME={quote(remote_root + '/.opencode-home')} ./bin/opencode export {quote(session_id)}"
                    exported = remote_shell(remote_host, export_cmd, timeout=120)
                    (raw_dir / "opencode_export.json").write_bytes(exported.stdout)
                    (raw_dir / "opencode_export.err").write_bytes(exported.stderr)
                    extra["opencode_session_id"] = session_id
    except urllib.error.URLError as error:
        stderr = str(error)
        returncode = 1
    finally:
        duration = time.time() - started
        (out_dir / "stdout.txt").write_text(stdout, encoding="utf-8")
        (out_dir / "stderr.txt").write_text(stderr, encoding="utf-8")
        try:
            copy_remote_workspace(remote_host, remote_workspace, workspace_dir)
        except Exception as error:
            stderr = stderr + f"\nworkspace copy failed: {error}\n"
            (out_dir / "stderr.txt").write_text(stderr, encoding="utf-8")

    metrics = {
        "platform": platform,
        "case_id": case_id,
        "repeat_index": int(repeat),
        "run_id": run_id,
        "status": "completed" if returncode == 0 else "failed",
        "returncode": returncode,
        "duration_seconds": round(duration, 3),
        "remote_workspace": remote_workspace,
        "output_dir": str(out_dir),
        **extra,
    }
    (out_dir / "metrics.json").write_text(json.dumps(metrics, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
