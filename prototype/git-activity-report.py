#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
git-activity-report — export commits as JSON (simple or sharded full), with natural-language windows,
optional GitHub PR enrichment, optional unmerged branch scanning, and local-time timestamps.

Key changes:
- ReportConfiguration dataclass: centralizes config (currently tz only, easy to extend).
- timestamps now includes: author epoch, commit epoch, author_local ISO, commit_local ISO, timezone ("local"|"utc").

Examples:
  # SIMPLE: last week, include unmerged branches, local time (default)
  git activity-report --simple --for "last week" --include-unmerged --repo . > last_week.json

  # FULL: last 6 months by month, PR enrichment, save patches to disk
  git activity-report --full --for "every month for the last 6 months" --repo . \
    --split-out out/last6 --save-patches out/last6/patches --github-prs
"""

from dataclasses import dataclass
import argparse, json, os, re, shutil, subprocess, sys
from datetime import datetime, timedelta, timezone

# ---------- configuration ----------


@dataclass
class ReportConfiguration:
    tz: str = "local"  # "local" or "utc"
    # In the future: include_patch defaults, max_patch_bytes, estimation settings, etc.


RC = ReportConfiguration()  # module-level singleton


def iso_in_tz(epoch, tz=None):
    tz = RC.tz if tz is None else tz
    dt = datetime.fromtimestamp(epoch, tz=timezone.utc)

    if tz == "local":
        dt = dt.astimezone()
    return dt.isoformat(timespec="seconds")


# ---------- subprocess helpers ----------


def run(cmd, cwd=None, text=True):
    p = subprocess.run(
        cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=text
    )
    if p.returncode != 0:
        raise RuntimeError(f"cmd failed: {' '.join(cmd)}\n{p.stderr}")
    return p.stdout if text else p.stdout.decode()


def git(root, *args):
    return run(["git", *args], cwd=root)


def short_sha(sha):
    return sha[:12]


# ---------- GitHub (quiet/optional) ----------


def parse_origin_github(root):
    try:
        url = git(root, "config", "--get", "remote.origin.url").strip()
    except Exception:
        return None
    if not url:
        return None
    m = re.match(
        r"(?:git@github\.com:|https?://github\.com/)([^/]+)/([^/]+?)(?:\.git)?$", url
    )
    if not m:
        return None
    return (m.group(1), m.group(2))


def gh_prs_for_commit(root, sha):
    """Return [] on any failure. If authenticated, returns minimal PR list."""
    owner_repo = parse_origin_github(root)

    if not owner_repo:
        return []
    owner, repo = owner_repo
    token = os.environ.get("GITHUB_TOKEN")
    try:
        if token:
            import urllib.request, json as _json

            url = f"https://api.github.com/repos/{owner}/{repo}/commits/{sha}/pulls"
            req = urllib.request.Request(
                url,
                headers={
                    "Accept": "application/vnd.github+json",
                    "Authorization": f"Bearer {token}",
                    "User-Agent": "git-activity-report",
                },
            )
            with urllib.request.urlopen(req) as r:
                prs = _json.loads(r.read().decode("utf-8"))
        elif shutil.which("gh"):
            out = run(
                [
                    "gh",
                    "api",
                    f"repos/{owner}/{repo}/commits/{sha}/pulls",
                    "-H",
                    "Accept: application/vnd.github+json",
                ]
            )
            prs = json.loads(out)
        else:
            return []
        out_list = []
        for pr in prs:
            html = pr.get("html_url")
            out_list.append(
                {
                    "number": pr.get("number"),
                    "title": pr.get("title"),
                    "state": pr.get("state"),
                    "created_at": pr.get("created_at"),
                    "merged_at": pr.get("merged_at"),
                    "html_url": html,
                    "diff_url": f"{html}.diff" if html else None,
                    "patch_url": f"{html}.patch" if html else None,
                    "user": {"login": (pr.get("user") or {}).get("login")},
                    "head": (pr.get("head") or {}).get("ref"),
                    "base": (pr.get("base") or {}).get("ref"),
                }
            )
        return out_list
    except Exception:
        return []


# ---------- time windows ----------


def iso(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%S")


def month_bounds(ym):
    y, m = [int(x) for x in ym.split("-")]
    assert 1 <= m <= 12
    start = datetime(y, m, 1)
    end = datetime(y + 1, 1, 1) if m == 12 else datetime(y, m + 1, 1)
    return iso(start), iso(end)


def start_of_week(dt):
    d0 = datetime(dt.year, dt.month, dt.day)
    return d0 - timedelta(days=d0.weekday())


def last_week_range(now=None):
    now = now or datetime.now()
    start_this_week = start_of_week(now)
    start_last_week = start_this_week - timedelta(days=7)
    return iso(start_last_week), iso(start_this_week)


def last_month_range(now=None):
    now = now or datetime.now()
    first_this = datetime(now.year, now.month, 1)
    first_last = (
        datetime(now.year - 1, 12, 1)
        if now.month == 1
        else datetime(now.year, now.month - 1, 1)
    )
    return iso(first_last), iso(first_this)


def last_n_months_calendar(n, now=None):
    now = now or datetime.now()
    y, m = now.year, now.month
    out = []
    cursor = datetime(y, m, 1)
    for _ in range(n):
        start = (
            datetime(cursor.year - 1, 12, 1)
            if cursor.month == 1
            else datetime(cursor.year, cursor.month - 1, 1)
        )
        end = cursor
        label = f"{start.year}-{start.month:02d}"
        out.append((label, iso(start), iso(end)))
        cursor = start
    out.reverse()
    return out


def last_n_weeks_calendar(n, now=None):
    now = now or datetime.now()
    this_week = start_of_week(now)
    cursor = this_week
    out = []
    for _ in range(n):
        start = cursor - timedelta(days=7)
        end = cursor
        year, weeknum, _ = start.isocalendar()
        label = f"{year}-W{weeknum:02d}"
        out.append((label, iso(start), iso(end)))
        cursor = start
    out.reverse()
    return out


def parse_for_phrase(for_str):
    s = for_str.strip().lower()
    m = re.match(r"every\s+month\s+for\s+the\s+last\s+(\d+)\s+months?", s)

    if m:
        return last_n_months_calendar(max(1, int(m.group(1)))), None
    m = re.match(r"every\s+week\s+for\s+the\s+last\s+(\d+)\s+weeks?", s)

    if m:
        return last_n_weeks_calendar(max(1, int(m.group(1)))), None
    if s == "last week":
        return [], last_week_range()
    if s == "last month":
        return [], last_month_range()
    # fallback: let git approxidate interpret since; until=now
    return [], (for_str, "now")


# ---------- git readers (HEAD commits) ----------


def commits_in_range(root, since, until, include_merges=False):
    args = [
        "-c",
        "log.showSignature=false",
        "rev-list",
        f"--since={since}",
        f"--until={until}",
        "--date-order",
        "--reverse",
        "HEAD",
    ]
    if not include_merges:
        args.insert(4, "--no-merges")
    out = git(root, *args)
    return [l.strip() for l in out.splitlines() if l.strip()]


def commit_meta(root, sha):
    fmt = "%H%x00%P%x00%an%x00%ae%x00%ad%x00%cN%x00%cE%x00%cD%x00%at%x00%ct%x00%s%x00%b"
    out = git(
        root, "show", "--no-patch", "--date=iso-strict", f"--pretty=format:{fmt}", sha
    )
    parts = out.split("\x00")
    (h, parents, an, ae, ad, cn, ce, cd, at, ct, subj, body) = (
        parts
        if len(parts) >= 12
        else (sha, "", "", "", "", "", "", "", "0", "0", "", "")
    )
    at_i = int(at)
    ct_i = int(ct)
    timestamps = {
        "author": at_i,
        "commit": ct_i,
        "author_local": iso_in_tz(at_i),
        "commit_local": iso_in_tz(ct_i),
        "timezone": RC.tz,
    }
    return {
        "sha": h,
        "parents": parents.split() if parents else [],
        "author": {"name": an, "email": ae, "date": ad},
        "committer": {"name": cn, "email": ce, "date": cd},
        "timestamps": timestamps,
        "subject": subj,
        "body": body,
    }


def commit_numstat(root, sha):
    out = git(root, "show", "--numstat", "--format=", "--no-color", sha)
    stats = {}
    files = []
    for line in out.splitlines():
        parts = line.split("\t")

        if len(parts) != 3:
            continue
        a, d, path = parts

        def to_int(x):
            try:
                return int(x)
            except:
                return None

        files.append({"file": path, "additions": to_int(a), "deletions": to_int(d)})
        stats[path] = (to_int(a), to_int(d))
    return files, stats


def commit_name_status(root, sha):
    out = subprocess.run(
        ["git", "show", "--name-status", "-z", "--format=", "--no-color", sha],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    if out.returncode != 0:
        return []
    data = out.stdout.decode("utf-8", "replace")
    parts = data.split("\x00")
    res = []
    i = 0
    while i < len(parts) and parts[i]:
        code = parts[i]
        i += 1
        if code.startswith("R") or code.startswith("C"):
            if i + 1 >= len(parts):
                break
            oldp = parts[i]
            newp = parts[i + 1]
            i += 2
            res.append({"status": code, "old_path": oldp, "file": newp})
        else:
            if i >= len(parts):
                break
            p = parts[i]
            i += 1
            if p == "":
                continue
            res.append({"status": code, "file": p})
    return res


def commit_shortstat(root, sha):
    out = (
        git(root, "show", "--shortstat", "--format=", "--no-color", sha)
        .strip()
        .splitlines()
    )
    return out[-1] if out else ""


def commit_patch(root, sha, max_bytes=0):
    txt = git(root, "show", "--patch", "--format=", "--no-color", sha)
    
    if max_bytes is None or max_bytes <= 0:
        return txt, False
    enc = txt.encode("utf-8")

    if len(enc) <= max_bytes:
        return txt, False
    enc = enc[:max_bytes]
    return enc.decode("utf-8", "ignore"), True


# ---------- branch scanning (UNMERGED) ----------


def current_branch(root):
    try:
        name = git(root, "rev-parse", "--abbrev-ref", "HEAD").strip()
        return None if name == "HEAD" else name
    except Exception:
        return None


def list_local_branches(root):
    out = git(root, "for-each-ref", "refs/heads", "--format=%(refname:short)")
    return [l.strip() for l in out.splitlines() if l.strip()]


def branch_ahead_behind(root, branch):
    out = git(root, "rev-list", "--left-right", "--count", f"HEAD...{branch}").strip()
    try:
        left, right = [int(x) for x in out.split()]
        return {"behind_head": left, "ahead_of_head": right}
    except Exception:
        return {"behind_head": None, "ahead_of_head": None}


def branch_merged_into_head(root, branch):
    try:
        rc = subprocess.run(
            ["git", "merge-base", "--is-ancestor", branch, "HEAD"],
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        return rc == 0
    except Exception:
        return None


def unmerged_commits_in_range(root, branch, since, until, include_merges):
    args = [
        "-c",
        "log.showSignature=false",
        "rev-list",
        branch,
        "^HEAD",
        f"--since={since}",
        f"--until={until}",
        "--date-order",
        "--reverse",
    ]
    if not include_merges:
        args.insert(4, "--no-merges")
    out = git(root, *args)
    return [l.strip() for l in out.splitlines() if l.strip()]


# ---------- writing ----------


def write_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2)


def save_patch_file(root, sha, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    p = os.path.join(out_dir, f"{short_sha(sha)}.patch")
    txt = git(root, "show", "--patch", "--format=", "--no-color", sha)
    with open(p, "w", encoding="utf-8") as f:
        f.write(txt)
    return p


# ---------- reporting core ----------


def build_commit_obj(
    root, sha, include_patch, max_patch_bytes, save_patches, github_prs
):
    meta = commit_meta(root, sha)
    files, num_map = commit_numstat(root, sha)
    ns = commit_name_status(root, sha)
    pretty = commit_shortstat(root, sha)

    files_detailed = []

    if ns:
        for entry in ns:
            path = entry.get("file")
            adds, dels = num_map.get(path, (None, None))
            item = {
                "file": path,
                "status": entry["status"],
                "additions": adds,
                "deletions": dels,
            }
            if "old_path" in entry:
                item["old_path"] = entry["old_path"]
            files_detailed.append(item)
    else:
        for f in files:
            files_detailed.append(
                {
                    "file": f["file"],
                    "status": "M",
                    "additions": f["additions"],
                    "deletions": f["deletions"],
                }
            )

    obj = {
        **meta,
        "short_sha": short_sha(meta["sha"]),
        "files": files_detailed,
        "diffstat_text": pretty,
        "patch_ref": {
            "embed": bool(include_patch),
            "git_show_cmd": [
                "git",
                "show",
                "--patch",
                "--format=",
                "--no-color",
                meta["sha"],
            ],
            "local_patch_file": None,
            "github_diff_url": None,
            "github_patch_url": None,
        },
    }

    if github_prs:
        prs = gh_prs_for_commit(root, meta["sha"])

        if prs:
            obj["github_prs"] = prs
            obj["patch_ref"]["github_diff_url"] = prs[0].get("diff_url")
            obj["patch_ref"]["github_patch_url"] = prs[0].get("patch_url")

    if include_patch:
        patch, clipped = commit_patch(root, meta["sha"], max_patch_bytes)
        obj["patch"] = patch
        obj["patch_clipped"] = bool(clipped)

    if save_patches:
        obj["patch_ref"]["local_patch_file"] = save_patch_file(
            root, meta["sha"], save_patches
        )

    return obj


def accumulate_manifest(manifest, commit_obj):
    akey = f"{commit_obj['author']['name']} <{commit_obj['author']['email']}>"
    manifest["authors"][akey] = manifest["authors"].get(akey, 0) + 1
    for f in commit_obj["files"]:
        manifest["summary"]["additions"] += f["additions"] or 0
        manifest["summary"]["deletions"] += f["deletions"] or 0
        manifest["summary"]["_files_touched_set"].add(f["file"])


def finalize_manifest(manifest):
    s = manifest["summary"]
    s["files_touched"] = len(s.pop("_files_touched_set", set()))


def report_for_range(
    root,
    label,
    since,
    until,
    mode_full,
    split_out,
    include_merges,
    include_patch,
    max_patch_bytes,
    save_patches,
    github_prs,
    include_unmerged,
    out_path="-",
):
    commits = commits_in_range(root, since, until, include_merges=include_merges)

    manifest = {
        "label": label,
        "range": {"since": since, "until": until},
        "repo": root,
        "include_merges": include_merges,
        "include_patch": include_patch,
        "mode": "full" if mode_full else "simple",
        "count": len(commits),
        "authors": {},
        "summary": {"additions": 0, "deletions": 0, "_files_touched_set": set()},
        "items": [],  # filenames (full) or inline commits (simple)
    }

    unmerged_section = None

    if mode_full:
        base = split_out or f"activity-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
        subdir = os.path.join(base, label or "window")
        os.makedirs(subdir, exist_ok=True)

        for sha in commits:
            obj = build_commit_obj(
                root,
                sha,
                include_patch,
                max_patch_bytes,
                os.path.join(subdir, "patches") if save_patches else None,
                github_prs,
            )
            accumulate_manifest(manifest, obj)
            ts = datetime.fromtimestamp(
                obj["timestamps"]["commit"], tz=timezone.utc
            ).astimezone()
            fname = f"{ts.strftime('%Y.%m.%d')}-{ts.strftime('%H.%M')}-{obj['short_sha']}.json"
            write_json(os.path.join(subdir, fname), obj)
            manifest["items"].append(
                {
                    "sha": obj["sha"],
                    "file": os.path.join(label or "window", fname),
                    "subject": obj["subject"],
                }
            )

        # Unmerged scanning (branches)
        if include_unmerged:
            cur = current_branch(root)
            branches = [b for b in list_local_branches(root) if b != cur]
            unmerged_section = {
                "branches_scanned": len(branches),
                "branches": [],
                "total_unmerged_commits": 0,
            }
            for br in branches:
                uniq_shas = unmerged_commits_in_range(
                    root, br, since, until, include_merges
                )
                if not uniq_shas:
                    continue
                merged = branch_merged_into_head(root, br)
                aheadbehind = branch_ahead_behind(root, br)
                br_dir = os.path.join(subdir, "unmerged", br.replace("/", "__"))
                os.makedirs(br_dir, exist_ok=True)
                items = []
                for sha in uniq_shas:
                    obj = build_commit_obj(
                        root,
                        sha,
                        include_patch,
                        max_patch_bytes,
                        os.path.join(br_dir, "patches") if save_patches else None,
                        github_prs,
                    )
                    ts = datetime.fromtimestamp(
                        obj["timestamps"]["commit"], tz=timezone.utc
                    ).astimezone()
                    fname = f"{ts.strftime('%Y.%m.%d')}-{ts.strftime('%H.%M')}-{obj['short_sha']}.json"
                    write_json(os.path.join(br_dir, fname), obj)
                    items.append(
                        {
                            "sha": obj["sha"],
                            "file": os.path.join(
                                label or "window",
                                "unmerged",
                                br.replace("/", "__"),
                                fname,
                            ),
                            "subject": obj["subject"],
                        }
                    )
                unmerged_section["branches"].append(
                    {
                        "name": br,
                        "merged_into_head": merged,
                        "ahead_of_head": aheadbehind["ahead_of_head"],
                        "behind_head": aheadbehind["behind_head"],
                        "items": items,
                    }
                )
                unmerged_section["total_unmerged_commits"] += len(items)

        finalize_manifest(manifest)
        if unmerged_section:
            manifest["unmerged_activity"] = unmerged_section
        write_json(os.path.join(base, f"manifest-{label or 'window'}.json"), manifest)
        return {"dir": base, "manifest": f"manifest-{label or 'window'}.json"}

    else:
        # simple: single JSON array
        commits_out = []

        for sha in commits:
            obj = build_commit_obj(
                root, sha, include_patch, max_patch_bytes, save_patches, github_prs
            )
            accumulate_manifest(manifest, obj)
            commits_out.append(obj)

        # Unmerged scanning (branches)
        if include_unmerged:
            cur = current_branch(root)
            branches = [b for b in list_local_branches(root) if b != cur]
            unmerged_section = {
                "branches_scanned": len(branches),
                "branches": [],
                "total_unmerged_commits": 0,
            }
            for br in branches:
                uniq_shas = unmerged_commits_in_range(
                    root, br, since, until, include_merges
                )
                if not uniq_shas:
                    continue
                merged = branch_merged_into_head(root, br)
                aheadbehind = branch_ahead_behind(root, br)
                br_commits = []
                for sha in uniq_shas:
                    br_commits.append(
                        build_commit_obj(
                            root,
                            sha,
                            include_patch,
                            max_patch_bytes,
                            save_patches,
                            github_prs,
                        )
                    )
                unmerged_section["branches"].append(
                    {
                        "name": br,
                        "merged_into_head": merged,
                        "ahead_of_head": aheadbehind["ahead_of_head"],
                        "behind_head": aheadbehind["behind_head"],
                        "commits": br_commits,
                    }
                )
                unmerged_section["total_unmerged_commits"] += len(br_commits)

        finalize_manifest(manifest)
        out = {**manifest, "commits": commits_out}
        if unmerged_section:
            out["unmerged_activity"] = unmerged_section

        if out_path == "-":
            json.dump(out, sys.stdout, indent=2)
            sys.stdout.write("\n")
        else:
            write_json(out_path, out)
        return {"file": out_path}


# ---------- main ----------


def main():
    ap = argparse.ArgumentParser(
        prog="git activity-report",
        description="Generate activity reports (JSON): simple or sharded full mode, natural-language windows, optional PR enrichment, optional unmerged-branch scan.",
    )
    ap.add_argument("--repo", default=".", help="Path to git repo (default: .)")

    # Time selection (choose one of --month | --for | --since/--until)
    ap.add_argument("--month", help="YYYY-MM (calendar month)")
    ap.add_argument(
        "--for",
        dest="for_str",
        help='Natural window, e.g. "last week", "every month for the last 6 months"',
    )
    ap.add_argument("--since", help="Custom since (ISO-ish or git approxidate)")
    ap.add_argument("--until", help="Custom until (exclusive)")

    # Mode
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument(
        "--simple", action="store_true", help="Single JSON output (quick)"
    )
    mode.add_argument(
        "--full",
        action="store_true",
        help="Sharded output (per-commit files + manifest)",
    )

    # Content switches
    ap.add_argument(
        "--include-merges", action="store_true", help="Include merge commits"
    )
    ap.add_argument(
        "--include-patch",
        action="store_true",
        help="Embed unified patches in JSON (big)",
    )
    ap.add_argument(
        "--max-patch-bytes",
        type=int,
        default=0,
        help="Per-commit patch cap (0=no limit default)",
    )
    ap.add_argument(
        "--save-patches", help="Directory to write .patch files (referenced in JSON)"
    )

    # Output layout
    ap.add_argument(
        "--split-out",
        help="Base directory for sharded output (required for --full). If omitted, auto-named.",
    )
    ap.add_argument("--out", default="-", help="File for --simple (default stdout)")

    # Integrations
    ap.add_argument(
        "--github-prs",
        action="store_true",
        help="Try to enrich with GitHub PRs (quietly ignored if not available)",
    )

    # Unmerged branches
    ap.add_argument(
        "--include-unmerged",
        action="store_true",
        help="Scan local branches for commits in the window not reachable from HEAD; include separately.",
    )

    # Timezone (ReportConfiguration)
    ap.add_argument(
        "--tz",
        choices=["local", "utc"],
        default="local",
        help="Timezone used for local ISO timestamps in output (default: local)",
    )

    if len(sys.argv) == 1:
        ap.print_help(sys.stderr)
        sys.exit(1)

    args = ap.parse_args()
    RC.tz = args.tz  # single point of truth

    # Determine windows
    buckets = []
    single = None

    if args.month:
        single = month_bounds(args.month)
    elif args.for_str:
        buckets, single = parse_for_phrase(args.for_str)
    elif args.since and args.until:
        single = (args.since, args.until)
    else:
        ap.error("Provide one of --month, --for, or (--since AND --until)")

    root = os.path.abspath(args.repo)
    mode_full = bool(args.full)

    if not args.simple and not args.full:
        mode_full = False  # default to simple

    if buckets:
        top = args.split_out or f"activity-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
        os.makedirs(top, exist_ok=True)
        top_manifest = {
            "repo": root,
            "multi": True,
            "buckets": [],
            "generated_at": datetime.now().isoformat(timespec="seconds"),
            "mode": "full" if mode_full else "simple",
            "include_merges": args.include_merges,
            "include_patch": args.include_patch,
            "include_unmerged": args.include_unmerged,
        }
        for label, since, until in buckets:
            if mode_full:
                res = report_for_range(
                    root,
                    label,
                    since,
                    until,
                    True,
                    top,
                    args.include_merges,
                    args.include_patch,
                    args.max_patch_bytes,
                    args.save_patches,
                    args.github_prs,
                    args.include_unmerged,
                    out_path="-",
                )
                top_manifest["buckets"].append(
                    {
                        "label": label,
                        "range": {"since": since, "until": until},
                        "manifest": res.get("manifest"),
                        "dir": res.get("dir", top),
                    }
                )
            else:
                fpath = os.path.join(top, f"{label}.json")
                res = report_for_range(
                    root,
                    label,
                    since,
                    until,
                    False,
                    None,
                    args.include_merges,
                    args.include_patch,
                    args.max_patch_bytes,
                    args.save_patches,
                    args.github_prs,
                    args.include_unmerged,
                    out_path=fpath,
                )
                top_manifest["buckets"].append(
                    {
                        "label": label,
                        "range": {"since": since, "until": until},
                        "file": fpath,
                    }
                )
        write_json(os.path.join(top, "manifest.json"), top_manifest)
        print(json.dumps({"dir": top, "manifest": "manifest.json"}, indent=2))
        return

    since, until = single
    label = None

    if args.month:
        label = args.month
    elif args.for_str in ("last week", "last month"):
        label = args.for_str.replace(" ", "-")
    else:
        label = "window"

    if mode_full:
        res = report_for_range(
            root,
            label,
            since,
            until,
            True,
            args.split_out,
            args.include_merges,
            args.include_patch,
            args.max_patch_bytes,
            args.save_patches,
            args.github_prs,
            args.include_unmerged,
            out_path="-",
        )
        print(json.dumps(res, indent=2))
    else:
        report_for_range(
            root,
            label,
            since,
            until,
            False,
            None,
            args.include_merges,
            args.include_patch,
            args.max_patch_bytes,
            args.save_patches,
            args.github_prs,
            args.include_unmerged,
            out_path=args.out,
        )


if __name__ == "__main__":
    main()
