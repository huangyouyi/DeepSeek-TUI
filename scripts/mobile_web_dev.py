#!/usr/bin/env python3
"""Start or print mobile Web SSH simulator development commands."""

from __future__ import annotations

import argparse
import os
import shutil
import shlex
import signal
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
MOBILE_WEB_DIR = REPO_ROOT / "mobile-web"


def quote_command(argv: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in argv)


def redact_command(argv: list[str]) -> list[str]:
    display: list[str] = []
    redact_next = False
    for item in argv:
        if redact_next:
            display.append("REDACTED")
            redact_next = False
            continue
        display.append(item)
        if item == "--access-token":
            redact_next = True
    return display


def quote_display_command(argv: list[str]) -> str:
    return quote_command(redact_command(argv))


def server_command(args: argparse.Namespace) -> list[str]:
    command = [
        "cargo",
        "run",
        "-p",
        "deepseek-mobile-web-server",
        "--",
        "--host",
        args.host,
        "--port",
        str(args.port),
        "--ssh-host",
        args.ssh_host,
        "--ssh-user",
        args.ssh_user,
        "--ssh-port",
        str(args.ssh_port),
    ]
    if args.use_real_model:
        command.append("--use-real-model")
    if args.access_token:
        command.extend(["--access-token", args.access_token])
    return command


def web_command(args: argparse.Namespace) -> list[str]:
    return [
        "npm",
        "run",
        "dev",
        "--",
        "--host",
        args.web_host,
        "--port",
        str(args.web_port),
    ]


def print_commands(args: argparse.Namespace) -> None:
    print("Mobile Web SSH simulator development commands")
    print()
    print("# Rust server")
    print(quote_display_command(server_command(args)))
    if not args.no_web:
        print()
        print("# Web dev server")
        if MOBILE_WEB_DIR.exists():
            print(f"cd {shlex.quote(str(MOBILE_WEB_DIR.relative_to(REPO_ROOT)))}")
            print(quote_display_command(web_command(args)))
        else:
            print("# mobile-web is not present yet; create/install it before running:")
            print(f"# cd {shlex.quote(str(MOBILE_WEB_DIR.relative_to(REPO_ROOT)))}")
            print(f"# {quote_display_command(web_command(args))}")
    print()
    print("# Smoke once the Rust server is listening")
    smoke = [
        "python3",
        "scripts/mobile_web_ssh_smoke.py",
        "--server",
        f"http://127.0.0.1:{args.port}",
    ]
    flow = [
        "python3",
        "scripts/mobile_web_ssh_flow_simulator.py",
        "--server",
        f"http://127.0.0.1:{args.port}",
        "--auto-approve",
        "--json",
    ]
    if args.access_token:
        smoke.extend(["--access-token", args.access_token])
        flow.extend(["--access-token", args.access_token])
    print(quote_display_command(smoke))
    print(quote_display_command(flow))


def ensure_dependencies(args: argparse.Namespace) -> list[str]:
    missing: list[str] = []
    if shutil.which("cargo") is None:
        missing.append("cargo")
    if not args.no_web and shutil.which("npm") is None:
        missing.append("npm")
    if not args.no_web and not MOBILE_WEB_DIR.exists():
        missing.append("mobile-web directory")
    return missing


def start_process(argv: list[str], cwd: Path, name: str) -> subprocess.Popen[bytes]:
    print(f"starting {name}: {quote_display_command(argv)}")
    return subprocess.Popen(argv, cwd=cwd)


def run_processes(args: argparse.Namespace) -> int:
    missing = ensure_dependencies(args)
    if missing:
        print("Cannot start development servers because dependencies are missing:")
        for item in missing:
            print(f"- {item}")
        print()
        print_commands(args)
        return 1

    processes: list[subprocess.Popen[bytes]] = []
    stopping = False

    def stop(_signum: int, _frame: object) -> None:
        nonlocal stopping
        stopping = True
        for process in processes:
            if process.poll() is None:
                process.terminate()

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)

    processes.append(start_process(server_command(args), REPO_ROOT, "Rust server"))
    if not args.no_web:
        processes.append(start_process(web_command(args), MOBILE_WEB_DIR, "Web dev server"))

    exit_code = 0
    while processes:
        for process in list(processes):
            code = process.poll()
            if code is not None:
                processes.remove(process)
                if code != 0 and not stopping:
                    exit_code = code
                    stop(signal.SIGTERM, None)
        if processes:
            try:
                os.wait()
            except ChildProcessError:
                break
            except InterruptedError:
                continue
    return exit_code


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Print or start mobile Web SSH simulator development commands."
    )
    parser.add_argument("--host", default="0.0.0.0", help="Rust server bind host")
    parser.add_argument("--port", type=int, default=8788, help="Rust server bind port")
    parser.add_argument("--ssh-host", default="192.168.30.244", help="SSH target host")
    parser.add_argument("--ssh-user", default="root", help="SSH target user")
    parser.add_argument("--ssh-port", type=int, default=22, help="SSH target port")
    parser.add_argument("--access-token", help="optional mobile web access token for the Rust server")
    parser.add_argument("--web-host", default="0.0.0.0", help="Web dev server bind host")
    parser.add_argument("--web-port", type=int, default=5173, help="Web dev server bind port")
    parser.add_argument("--use-real-model", action="store_true", help="pass --use-real-model to the Rust server")
    parser.add_argument("--no-web", action="store_true", help="only run or print the Rust server command")
    parser.add_argument("--start", action="store_true", help="start processes instead of printing commands")
    parser.add_argument("--print", action="store_true", help="print commands and exit")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.print or not args.start:
        print_commands(args)
        return 0
    return run_processes(args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
