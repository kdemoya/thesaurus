#!/usr/bin/env python3
"""Exercise an assembled app's local service using disposable synthetic data.

This opens no GUI, starts no model/provider requests, and captures no devices.
The report contains checks and counts; service output and instance tokens remain
inside the automatically removed temporary workspace.
"""

import argparse
import json
import os
from pathlib import Path
import plistlib
import signal
import stat
import subprocess
import sys
import tempfile
import time
from urllib.parse import urlsplit
from urllib.request import ProxyHandler, Request, build_opener


class SmokeFailure(Exception):
    """A fixed diagnostic code that cannot expose child output or tokens."""


class Smoke:
    def __init__(self, app, root, report):
        self.resources = app / "Contents/Resources/resources"
        self.service = self.resources / "thesaurus"
        self.workspace = root / "workspace"
        self.report = report
        self.process = None
        self.instance = None
        self.log = None
        # Preserve the actual HOME rather than redirecting account defaults.
        # No provider credentials or development tool paths enter the child.
        self.environment = {
            key: os.environ[key]
            for key in ("HOME", "USER", "LOGNAME", "LANG", "LC_ALL")
            if key in os.environ
        }
        self.environment["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
        self.environment["TMPDIR"] = str(root / "tmp")
        (root / "tmp").mkdir(mode=0o700)
        self.http = build_opener(ProxyHandler({}))

    def check(self, name, condition):
        self.report["checks"][name] = bool(condition)
        if not condition:
            raise SmokeFailure(name)

    def cli(self, *args, parse_json=True, expect_success=True):
        try:
            result = subprocess.run(
                [str(self.service), "--data-dir", str(self.workspace), *args],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=self.environment,
                cwd=self.workspace.parent,
                timeout=40,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise SmokeFailure("cli_timeout") from error
        if not expect_success:
            self.check("unaccepted_export_blocked", result.returncode != 0)
            return None
        if result.returncode != 0:
            raise SmokeFailure("cli_command_failed")
        if not parse_json:
            return None
        try:
            return json.loads(result.stdout)
        except (ValueError, UnicodeError) as error:
            raise SmokeFailure("cli_invalid_json") from error

    def api(self, path, body=None, method=None):
        request = Request(
            self.instance["api_url"] + path,
            data=json.dumps(body).encode() if body is not None else None,
            headers={
                "Authorization": "Bearer " + self.instance["token"],
                "Content-Type": "application/json",
            },
            method=method,
        )
        with self.http.open(request, timeout=5) as response:
            return json.load(response)

    def start(self):
        self.log = tempfile.TemporaryFile()
        self.process = subprocess.Popen(
            [
                str(self.service), "--data-dir", str(self.workspace),
                "serve", "--port", "0", "--web-dir",
                str(self.resources / "web"),
            ],
            stdin=subprocess.DEVNULL,
            stdout=self.log,
            stderr=self.log,
            env=self.environment,
            cwd=self.workspace.parent,
            start_new_session=True,
        )
        deadline = time.monotonic() + 30
        instance_path = self.workspace / "instance.json"
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise SmokeFailure("service_exited_during_startup")
            try:
                instance = json.loads(instance_path.read_bytes())
                url = urlsplit(instance["api_url"])
                if (
                    instance["pid"] != self.process.pid
                    or url.scheme != "http"
                    or url.hostname != "127.0.0.1"
                    or not url.port
                    or Path(instance["socket"]).parent != self.workspace
                    or not isinstance(instance["token"], str)
                ):
                    raise SmokeFailure("invalid_owned_service_instance")
                self.instance = instance
                if self.api("/health").get("status") == "ready":
                    self.check(
                        "instance_file_private",
                        stat.S_IMODE(instance_path.stat().st_mode) == 0o600,
                    )
                    return
            except (OSError, ValueError, KeyError):
                pass
            time.sleep(0.1)
        raise SmokeFailure("service_startup_timeout")

    def stop(self):
        if self.process is None:
            return True
        process = self.process
        try:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
            graceful = process.returncode == 0
            return graceful and not (self.workspace / "instance.json").exists()
        finally:
            self.instance = None
            self.process = None
            if self.log is not None:
                self.log.close()
                self.log = None

    def exercise(self):
        self.check("bundled_service_present", self.service.is_file())
        self.check("bundled_web_present", (self.resources / "web/index.html").is_file())
        self.cli("init", parse_json=False)
        self.check("workspace_initialized", (self.workspace / "config.toml").is_file())
        self.start()
        self.check("system_path_startup", True)
        self.check(
            "database_private",
            stat.S_IMODE((self.workspace / "thesaurus.sqlite3").stat().st_mode) == 0o600,
        )
        with self.http.open(self.instance["api_url"] + "/", timeout=5) as response:
            self.check("bundled_web_served", b"<html" in response.read(200_000).lower())

        fixture = self.workspace.parent / "synthetic-release.md"
        fixture.write_text(
            "Accomplishment: Shipped the alphasentinel importer with synthetic validation.\n"
            "Impact: Reduced the synthetic workflow from ten steps to two.\n"
            "Achievement: Documented the synthetic release process.\n"
        )
        imported = self.cli("import", str(fixture), "--title", "Synthetic alpha release")
        document = imported["document"]
        self.check("cli_import", imported["changed"] is True)
        hits = self.cli("search", "alphasentinel")
        self.check("cli_search", any(hit["document_id"] == document["id"] for hit in hits))
        self.report["counts"]["search_hits"] = len(hits)

        plugin_path = "/api/v1/plugins/thesaurus.bragbook"
        plugin = self.api(plugin_path)
        self.check("bragbook_disabled_by_default", plugin["enabled"] is False)
        self.api(
            plugin_path,
            {
                "expected_version": plugin["version"], "enabled": True,
                "configuration": {
                    "completed-tasks": False, "confirmed-deliveries": False,
                    "source-markers": True,
                },
                "grant": {
                    "operations": ["sources.read"],
                    "source_ids": [document["source_id"]],
                },
                "idempotency_key": "synthetic-alpha-enable",
            },
            "PATCH",
        )
        self.cli("bragbook", "collect")
        candidates = self.cli("bragbook", "list", "--status", "candidate")
        self.check("bragbook_collection", candidates["total"] == 3)
        self.report["counts"]["collected_candidates"] = candidates["total"]
        entry = next(item for item in candidates["items"] if item["title"].startswith("Shipped"))
        self.check(
            "bragbook_pinned_evidence",
            bool(entry["evidence"]) and all(
                citation["document_id"] == document["id"]
                and citation["revision"] == document["revision"]
                and citation["line"] >= 1
                for citation in entry["evidence"]
            ),
        )
        self.cli("bragbook", "export", "--entry", entry["id"], expect_success=False)

        review = {
            "expected_version": entry["version"], "status": "candidate",
            "title": entry["title"],
            "contribution": "I implemented and validated this synthetic importer.",
            "impact": "The synthetic workflow decreased from ten steps to two.",
            "occurred_on": None, "idempotency_key": "synthetic-alpha-edit",
        }
        review_file = self.workspace.parent / "review.json"
        review_file.write_text(json.dumps(review))
        edited = self.cli("bragbook", "review", entry["id"], "--file", str(review_file))
        self.check(
            "bragbook_review_edit",
            edited["status"] == "candidate" and edited["contribution"] == review["contribution"],
        )
        review.update({
            "expected_version": edited["version"], "status": "accepted",
            "idempotency_key": "synthetic-alpha-accept",
        })
        review_file.write_text(json.dumps(review))
        accepted = self.cli("bragbook", "review", entry["id"], "--file", str(review_file))
        self.check("bragbook_accept", accepted["status"] == "accepted")

        for format_name, extension in (("json", "json"), ("markdown", "md")):
            output = self.workspace.parent / ("highlights." + extension)
            result = self.cli(
                "bragbook", "export", "--entry", entry["id"],
                "--format", format_name, "--output", str(output),
            )
            self.check("bragbook_" + format_name + "_export", result["entry_count"] == 1)
            self.check(
                format_name + "_export_private",
                stat.S_IMODE(output.stat().st_mode) == 0o600,
            )
            if format_name == "json":
                payload = json.loads(output.read_text())
                self.check(
                    "bragbook_export_content",
                    payload["schema"] == "thesaurus-bragbook/v1"
                    and len(payload["entries"]) == 1
                    and payload["entries"][0]["id"] == entry["id"]
                    and payload["entries"][0]["contribution"] == review["contribution"],
                )
            else:
                self.check("markdown_citation", document["id"] in output.read_text())
        self.check("bragbook_deduplication", self.cli("bragbook", "collect")["created"] == 0)
        self.check("first_graceful_shutdown", self.stop())

        self.start()
        hits = self.cli("search", "alphasentinel")
        self.check("source_survives_restart", any(hit["document_id"] == document["id"] for hit in hits))
        restored = self.cli("bragbook", "list", "--status", "accepted")
        self.check(
            "bragbook_survives_restart",
            restored["total"] == 1
            and restored["items"][0]["id"] == entry["id"]
            and restored["items"][0]["contribution"] == review["contribution"],
        )
        restored_plugin = self.api(plugin_path)
        self.check(
            "plugin_scope_survives_restart",
            restored_plugin["enabled"] is True
            and restored_plugin["grant"]["source_ids"] == [document["source_id"]],
        )
        self.check("restart_deduplication", self.cli("bragbook", "collect")["created"] == 0)
        self.report["counts"].update({"accepted_entries": 1, "exported_entries": 1, "restarts": 1})
        self.check("final_graceful_shutdown", self.stop())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True, help="Assembled Thesaurus.app")
    parser.add_argument("--report", type=Path, required=True, help="Write sanitized JSON checks")
    args = parser.parse_args()
    report = {"schema": "thesaurus-macos-smoke/v1", "status": "failed", "checks": {}, "counts": {}}
    smoke = None

    def interrupted(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    try:
        app = args.app.resolve(strict=True)
        with (app / "Contents/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        report["bundle_version"] = str(info.get("CFBundleShortVersionString", "unknown"))
        with tempfile.TemporaryDirectory(prefix="thesaurus-alpha-", dir="/private/tmp") as temporary:
            smoke = Smoke(app, Path(temporary), report)
            try:
                smoke.exercise()
            finally:
                cleaned_up = smoke.stop()
                report["checks"]["owned_service_cleanup"] = cleaned_up
                if not cleaned_up:
                    raise SmokeFailure("owned_service_cleanup")
        report["status"] = "passed"
    except KeyboardInterrupt:
        report["failure"] = "interrupted"
    except SmokeFailure as error:
        report["failure"] = str(error)
    except Exception as error:
        # Never include exception text: network/subprocess errors can carry
        # absolute workspace paths, headers, or captured child output.
        report["failure"] = "unexpected_" + type(error).__name__
    args.report.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(args.report, os.O_CREAT | os.O_TRUNC | os.O_WRONLY, 0o600)
    os.fchmod(descriptor, 0o600)
    with os.fdopen(descriptor, "w") as file:
        json.dump(report, file, indent=2)
        file.write("\n")
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
