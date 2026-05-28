#!/usr/bin/env python3
import csv
import json
import os
import sys
from collections import defaultdict
from pathlib import Path


def env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise SystemExit(f"missing required env: {name}")
    return value


def main() -> int:
    runs_dir = Path(env("EVAL_RUNS_DIR"))
    reports_dir = Path(env("EVAL_REPORTS_DIR"))
    run_id = env("EVAL_RUN_ID")
    run_dir = runs_dir / run_id
    report_dir = reports_dir / run_id
    report_dir.mkdir(parents=True, exist_ok=True)

    rows = []
    for judge_path in sorted(run_dir.glob("*/*/*/judge.json")):
        summary_path = judge_path.parent / "summary.json"
        judge = json.loads(judge_path.read_text(encoding="utf-8"))
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        scores = judge.get("scores", {})
        row = {
            "platform": summary["platform"],
            "case_id": summary["case_id"],
            "repeat_index": summary["repeat_index"],
            "status": summary["status"],
            "duration_seconds": summary.get("duration_seconds"),
            "total": judge.get("total", 0),
            "task_completion": scores.get("task_completion", 0),
            "result_quality": scores.get("result_quality", 0),
            "efficiency": scores.get("efficiency", 0),
            "user_experience": scores.get("user_experience", 0),
            "stability_safety": scores.get("stability_safety", 0),
        }
        rows.append(row)

    csv_path = report_dir / "summary.csv"
    fields = [
        "platform",
        "case_id",
        "repeat_index",
        "status",
        "duration_seconds",
        "total",
        "task_completion",
        "result_quality",
        "efficiency",
        "user_experience",
        "stability_safety",
    ]
    with csv_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)

    totals = defaultdict(list)
    for row in rows:
        totals[row["platform"]].append(float(row["total"]))
    comparison = {
        platform: {
            "runs": len(values),
            "average_total": round(sum(values) / len(values), 2) if values else 0,
        }
        for platform, values in sorted(totals.items())
    }
    (report_dir / "platform-comparison.json").write_text(
        json.dumps(comparison, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    lines = [
        f"# Agent Eval Report: {run_id}",
        "",
        "| Platform | Runs | Average Total |",
        "|---|---:|---:|",
    ]
    for platform, data in comparison.items():
        lines.append(f"| {platform} | {data['runs']} | {data['average_total']} |")
    lines.extend(["", f"Raw CSV: `{csv_path.name}`"])
    (report_dir / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(report_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
