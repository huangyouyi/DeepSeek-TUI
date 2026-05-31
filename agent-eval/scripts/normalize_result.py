#!/usr/bin/env python3
import json
import os
import re
import sys
from pathlib import Path


def env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise SystemExit(f"missing required env: {name}")
    return value


def list_files(path: Path) -> list[str]:
    if not path.exists():
        return []
    return sorted(str(p.relative_to(path)) for p in path.rglob("*") if p.is_file())


def read_json(path: Path) -> dict | list | None:
    if not path.exists():
        return None
    text = path.read_text(encoding="utf-8", errors="replace")
    if path.name == "opencode_export.json" and text.startswith("Exporting session:"):
        text = text[text.find("{") :]
    if not text.lstrip().startswith(("{", "[")):
        start = text.find("{")
        end = text.rfind("}")
        if start >= 0 and end > start:
            text = text[start : end + 1]
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return None


def final_from_kai(data: dict | None) -> str:
    if not isinstance(data, dict):
        return ""
    turns = data.get("turns")
    if isinstance(turns, list):
        for turn in reversed(turns):
            if isinstance(turn, dict) and isinstance(turn.get("assistant_answer"), str):
                answer = turn["assistant_answer"].strip()
                if answer and "I need approval before running" not in answer:
                    return answer
    turn = data.get("turn")
    if isinstance(turn, dict) and isinstance(turn.get("assistant_answer"), str):
        return turn["assistant_answer"]
    messages = data.get("messages")
    if not isinstance(messages, list):
        return ""
    for message in reversed(messages):
        if message.get("role") == "assistant" or message.get("type") == "assistant":
            content = message.get("content") or message.get("text")
            if isinstance(content, str):
                return content
            parts = message.get("parts")
            if isinstance(parts, list):
                texts = [p.get("text", "") for p in parts if isinstance(p, dict) and p.get("text")]
                if texts:
                    return "\n".join(texts)
    return ""


def final_from_codewhale(data: dict | None) -> str:
    if isinstance(data, dict):
        return str(data.get("output") or data.get("final_answer") or "")
    return ""


def final_from_opencode(exported: dict | None) -> str:
    if not isinstance(exported, dict):
        return ""
    messages = exported.get("messages") or []
    for message in reversed(messages):
        info = message.get("info") or {}
        if info.get("role") != "assistant":
            continue
        texts = []
        for part in message.get("parts") or []:
            if part.get("type") == "text" and part.get("text"):
                texts.append(part["text"])
        if texts:
            return "\n".join(texts)
    return ""


def main() -> int:
    runs_dir = Path(env("EVAL_RUNS_DIR"))
    platform = env("PLATFORM")
    case_id = env("CASE_ID")
    repeat = env("REPEAT_INDEX")
    run_id = env("EVAL_RUN_ID")

    out_dir = runs_dir / run_id / platform / case_id / repeat
    metrics = read_json(out_dir / "metrics.json") or {}
    stdout_path = out_dir / "stdout.txt"
    stderr_path = out_dir / "stderr.txt"
    stdout = stdout_path.read_text(encoding="utf-8", errors="replace") if stdout_path.exists() else ""
    stderr = stderr_path.read_text(encoding="utf-8", errors="replace") if stderr_path.exists() else ""

    final_answer = ""
    tool_calls = []
    errors = []
    if platform == "kai":
        final_answer = final_from_kai(read_json(stdout_path))
    elif platform == "codewhale":
        final_answer = final_from_codewhale(read_json(stdout_path))
    elif platform == "opencode":
        final_answer = final_from_opencode(read_json(out_dir / "raw" / "opencode_export.json"))
        tool_calls = re.findall(r'"type"\s*:\s*"tool"', stdout)

    if stderr.strip():
        errors.append(stderr.strip()[-2000:])

    workspace = out_dir / "workspace"
    files = list_files(workspace)
    summary = {
        "platform": platform,
        "case_id": case_id,
        "repeat_index": int(repeat),
        "run_id": run_id,
        "status": metrics.get("status", "unknown"),
        "duration_seconds": metrics.get("duration_seconds"),
        "returncode": metrics.get("returncode"),
        "final_answer": final_answer,
        "created_files": files,
        "modified_files": [],
        "deleted_files": [],
        "tool_calls": tool_calls,
        "errors": errors,
        "raw_paths": {
            "stdout": "stdout.txt",
            "stderr": "stderr.txt",
            "workspace": "workspace",
        },
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
