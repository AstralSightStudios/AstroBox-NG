# -*- coding: utf-8 -*-
"""argparse wiring and command dispatch for abtools."""

import argparse
import sys
from pathlib import Path

from . import ui
from .commands import branch as branch_cmd
from .commands import profile as profile_cmd
from .commands.build import run_build
from .commands.commit import run_commit
from .commands.dev import run_dev
from .commands.init import run_init
from .commands.push import run_push
from .commands.sync import run_sync
from .commands.tui import run_tui


def _add_conflict_flags(p: argparse.ArgumentParser) -> None:
    """Shared smart-merge / conflict-resolution options for sync & push."""
    p.add_argument(
        "--prefer",
        choices=["interactive", "local", "remote", "abort"],
        default="interactive",
        help="Conflict policy git can't auto-resolve: interactive picker (default), "
        "keep local (-X ours), take upstream (-X theirs), or abort.",
    )
    p.add_argument(
        "--merge",
        dest="merge_mode",
        action="store_const",
        const="merge",
        default="rebase",
        help="Use merge instead of rebase for divergent history.",
    )
    p.add_argument(
        "--no-lockfile-auto",
        dest="auto_lockfiles",
        action="store_false",
        default=True,
        help="Don't auto-resolve lockfiles (Cargo.lock/pnpm-lock.yaml) to upstream.",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="abtools",
        description="Lightweight CLI to sync multiple repos and run builds for AstroBox.",
        add_help=False,  # we provide a unified 'help' subcommand
    )

    parser.add_argument("-h", "--help", action="store_true", help="Show help")
    parser.add_argument(
        "-f", "--file", default="repos.xml", help="Path to repos.xml (default: repos.xml)"
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose logging")

    subparsers = parser.add_subparsers(dest="command")

    p_init = subparsers.add_parser("init", help="Run sync, then custom extras")
    p_init.add_argument(
        "--private", action="store_true", help="Include private repos (requires configured git creds)"
    )

    p_sync = subparsers.add_parser("sync", help="Clone/update repos from repos.xml")
    p_sync.add_argument(
        "--private", action="store_true", help="Include private repos (requires configured git creds)"
    )
    _add_conflict_flags(p_sync)

    p_build = subparsers.add_parser("build", help="Run build by target")
    p_build.add_argument("target", nargs="?", help="Build target, e.g. android / ios / wasm")
    p_build.add_argument(
        "extra", nargs=argparse.REMAINDER, help="Extra args passed to target branch"
    )

    p_dev = subparsers.add_parser(
        "dev", help="Verify the pinned Cargo workspace, then run the app (--tauri / --wasm)"
    )
    p_dev.add_argument("--tauri", action="store_true", help="Run tauri app development")
    p_dev.add_argument("--wasm", action="store_true", help="Run web dev server with wasm backend")

    p_commit = subparsers.add_parser("commit", help="Commit root repo and all sub repos")
    p_commit.add_argument(
        "-m",
        "--message",
        help="Commit message reused for every repo (opens $EDITOR per repo if omitted)",
    )

    p_push = subparsers.add_parser("push", help="Push root repo and all sub repos")
    _add_conflict_flags(p_push)

    # profile
    p_profile = subparsers.add_parser("profile", help="Manage & apply shared dev configs (repo→branch)")
    psub = p_profile.add_subparsers(dest="profile_cmd")
    psub.add_parser("list", help="List dev profiles")
    p_show = psub.add_parser("show", help="Show one profile's overrides")
    p_show.add_argument("name")
    p_save = psub.add_parser("save", help="Save current branches (and/or --set) as a profile")
    p_save.add_argument("name")
    p_save.add_argument("--set", action="append", default=[], metavar="REPO=BRANCH", help="Add/override an entry")
    p_save.add_argument("--no-from-current", dest="from_current", action="store_false", default=True,
                        help="Don't snapshot current branches")
    p_del = psub.add_parser("delete", help="Delete a profile")
    p_del.add_argument("name")
    p_apply = psub.add_parser("apply", help="Apply a profile (switch branches) and sync")
    p_apply.add_argument("name")
    p_apply.add_argument("--no-sync", dest="do_sync", action="store_false", default=True)
    _add_conflict_flags(p_apply)
    p_clear = psub.add_parser("clear", help="Deactivate profile, back to default branches")
    p_clear.add_argument("--no-sync", dest="do_sync", action="store_false", default=True)
    _add_conflict_flags(p_clear)
    psub.add_parser("current", help="Show active profile and per-repo branch status")

    # branch
    p_branch = subparsers.add_parser("branch", help="Create/list branches across repos")
    bsub = p_branch.add_subparsers(dest="branch_cmd")
    p_bnew = bsub.add_parser("new", help="Create a branch in one repo (--repo) or all (--all)")
    p_bnew.add_argument("name")
    p_bnew.add_argument("--all", action="store_true", help="Create in every repo")
    p_bnew.add_argument("--repo", help="Repo key/name for a single-repo branch")
    p_bnew.add_argument("--base", help="Base branch (default: main for --all, else repo default)")
    p_bnew.add_argument("--push", action="store_true", help="Also push -u to origin immediately")
    bsub.add_parser("list", help="List the current branch of every repo")

    subparsers.add_parser("tui", help="Interactive dashboard + action launcher (curses)")
    subparsers.add_parser("help", help="Print command help")

    return parser


def _dispatch_profile(args, xml_path: Path) -> int:
    cmd = getattr(args, "profile_cmd", None)
    if cmd == "list":
        return profile_cmd.list_profiles(xml_path)
    if cmd == "show":
        return profile_cmd.show_profile(xml_path, args.name)
    if cmd == "save":
        return profile_cmd.save_profile(xml_path, args.name, args.set, args.from_current)
    if cmd == "delete":
        return profile_cmd.delete_profile(xml_path, args.name)
    if cmd == "apply":
        return profile_cmd.apply_profile(
            xml_path, args.name, do_sync=args.do_sync, policy=args.prefer,
            merge_mode=args.merge_mode, auto_lockfiles=args.auto_lockfiles, verbose=args.verbose,
        )
    if cmd == "clear":
        return profile_cmd.clear_profile(
            xml_path, do_sync=args.do_sync, policy=args.prefer,
            merge_mode=args.merge_mode, auto_lockfiles=args.auto_lockfiles, verbose=args.verbose,
        )
    if cmd == "current":
        return profile_cmd.current_profile(xml_path)
    ui.err("Usage: python abtools.py profile {list|show|save|delete|apply|clear|current}")
    return 2


def _dispatch_branch(args, xml_path: Path) -> int:
    cmd = getattr(args, "branch_cmd", None)
    if cmd == "new":
        return branch_cmd.branch_new(
            xml_path, args.name, all_repos=args.all, repo=args.repo,
            base=args.base, push=args.push, verbose=args.verbose,
        )
    if cmd == "list":
        return branch_cmd.branch_list(xml_path)
    ui.err("Usage: python abtools.py branch {new|list}")
    return 2


def print_help_and_exit(parser: argparse.ArgumentParser) -> None:
    ui.out("✨ ABTools · Multi-repository management tool for AstroBox-NG")
    print(parser.format_help())
    sys.exit(0)


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if args.help:
        print_help_and_exit(parser)

    xml_path = Path(args.file).resolve()

    if not args.command:
        # Bare invocation in a real terminal lands on the TUI; otherwise help.
        if sys.stdin.isatty() and sys.stdout.isatty():
            sys.exit(run_tui(xml_path, args.verbose))
        print_help_and_exit(parser)

    if args.command == "init":
        rc = run_init(xml_path, include_private=args.private, verbose=args.verbose)
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "sync":
        rc = run_sync(
            xml_path,
            include_private=args.private,
            verbose=args.verbose,
            policy=args.prefer,
            merge_mode=args.merge_mode,
            auto_lockfiles=args.auto_lockfiles,
        )
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "build":
        rc = run_build(args.target, args.extra)
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "dev":
        rc = run_dev(xml_path, verbose=args.verbose, tauri=args.tauri, wasm=args.wasm)
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "commit":
        rc = run_commit(xml_path, args.message, args.verbose)
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "push":
        rc = run_push(
            xml_path,
            args.verbose,
            policy=args.prefer,
            merge_mode=args.merge_mode,
            auto_lockfiles=args.auto_lockfiles,
        )
        ui.out("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "profile":
        sys.exit(_dispatch_profile(args, xml_path))

    elif args.command == "branch":
        sys.exit(_dispatch_branch(args, xml_path))

    elif args.command == "tui":
        sys.exit(run_tui(xml_path, args.verbose))

    elif args.command == "help":
        print_help_and_exit(parser)

    else:
        ui.err(f"Unknown command: {args.command}")
        print_help_and_exit(parser)
