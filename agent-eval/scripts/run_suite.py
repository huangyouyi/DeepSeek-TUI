#!/usr/bin/env python3
import os
import subprocess
import sys


def split_env(name: str) -> list[str]:
    return [item.strip() for item in os.environ.get(name, "").split(",") if item.strip()]


def main() -> int:
    scripts_dir = os.environ["EVAL_SCRIPTS_DIR"]
    platforms = split_env("EVAL_PLATFORMS")
    cases = split_env("EVAL_CASES")
    repeat_count = int(os.environ.get("EVAL_REPEAT_COUNT", "3"))
    failures = 0

    for platform in platforms:
        for case_id in cases:
            for repeat in range(1, repeat_count + 1):
                env = os.environ.copy()
                env.update({
                    "PLATFORM": platform,
                    "CASE_ID": case_id,
                    "REPEAT_INDEX": str(repeat),
                })
                for script in ["run_case.py", "normalize_result.py", "judge_result.py"]:
                    result = subprocess.run(
                        ["python3", f"{scripts_dir}/{script}"],
                        env=env,
                        check=False,
                    )
                    if result.returncode != 0:
                        failures += 1
                        break
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
