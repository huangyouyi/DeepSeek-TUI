#!/usr/bin/env python3
import json
import os
import sys
import urllib.request
from pathlib import Path


def env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise SystemExit(f"missing required env: {name}")
    return value


def read_text(path: Path, limit: int = 12000) -> str:
    if not path.exists():
        return ""
    text = path.read_text(encoding="utf-8", errors="replace")
    return text[:limit]


def deepseek_chat(api_key: str, model: str, user_prompt: str) -> dict:
    payload = {
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是严谨的 AI Agent 横向评测裁判。只输出合法 JSON，不要输出 Markdown。",
            },
            {"role": "user", "content": user_prompt},
        ],
        "stream": False,
        "temperature": 0.0,
    }
    request = urllib.request.Request(
        "https://api.deepseek.com/chat/completions",
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={
            "authorization": f"Bearer {api_key}",
            "content-type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=300) as response:
        body = json.loads(response.read().decode("utf-8"))
    content = body["choices"][0]["message"]["content"]
    return json.loads(content)


def main() -> int:
    runs_dir = Path(env("EVAL_RUNS_DIR"))
    cases_dir = Path(env("EVAL_CASES_DIR"))
    platform = env("PLATFORM")
    case_id = env("CASE_ID")
    repeat = env("REPEAT_INDEX")
    run_id = env("EVAL_RUN_ID")
    api_key = env("SECRET_DEEPSEEK_API_KEY")
    judge_model = os.environ.get("JUDGE_MODEL", "deepseek-v4-pro")

    out_dir = runs_dir / run_id / platform / case_id / repeat
    summary = json.loads((out_dir / "summary.json").read_text(encoding="utf-8"))
    prompt = read_text(cases_dir / case_id / "prompt.txt")
    rubric = json.loads((cases_dir / case_id / "rubric.json").read_text(encoding="utf-8"))
    files = "\n".join(summary.get("created_files", []))

    judge_prompt = f"""
请评估一个 AI Agent 的任务结果。

评分总分 100：
- task_completion: 0 到 35
- result_quality: 0 到 20
- efficiency: 0 到 20
- user_experience: 0 到 15
- stability_safety: 0 到 10

必须返回 JSON：
{{
  "scores": {{
    "task_completion": 0,
    "result_quality": 0,
    "efficiency": 0,
    "user_experience": 0,
    "stability_safety": 0
  }},
  "total": 0,
  "strengths": [],
  "weaknesses": [],
  "optimization_suggestions": []
}}

用户原始任务：
{prompt}

验收标准：
{json.dumps(rubric, ensure_ascii=False, indent=2)}

平台 summary：
{json.dumps(summary, ensure_ascii=False, indent=2)}

工作区文件列表：
{files}

请严格按证据评分。不要因为平台名称给额外偏见。不要输出 JSON 以外内容。
"""
    try:
        judge = deepseek_chat(api_key, judge_model, judge_prompt)
    except Exception as error:
        judge = {
            "scores": {
                "task_completion": 0,
                "result_quality": 0,
                "efficiency": 0,
                "user_experience": 0,
                "stability_safety": 0,
            },
            "total": 0,
            "strengths": [],
            "weaknesses": [f"judge failed: {error}"],
            "optimization_suggestions": [],
        }

    scores = judge.get("scores", {})
    judge["total"] = int(sum(int(scores.get(key, 0)) for key in [
        "task_completion",
        "result_quality",
        "efficiency",
        "user_experience",
        "stability_safety",
    ]))
    (out_dir / "judge.json").write_text(json.dumps(judge, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(judge, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
