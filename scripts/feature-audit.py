#!/usr/bin/env python3
"""Canonical feature audit: concurrent JSON imports and a read-only dashboard."""
import argparse
import csv
import fcntl
import json
import mimetypes
import os
from pathlib import Path
import re
import shutil
import tempfile
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import quote, unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
CSV = ROOT / "docs/feature-audit.csv"
ASSETS = ROOT / "docs/feature-audit"
AUDIT = ROOT / ".amp/in/audit"
SHOWCASE = ROOT / ".amp/in/artifacts/feature-audit/showcase"
FIELDS = "id,area,feature,user_story,expected_behavior,source,owner,status,test_command,evidence,errors,fix,retest,showcase,updated_at".split(",")
STATUSES = "inventoried testing failed fixing retesting passed unsupported blocked".split()
HISTORY = {"errors", "fix", "retest"}
SAFE = {".png", ".jpg", ".jpeg", ".webp", ".svg", ".pdf", ".txt", ".log", ".json", ".md", ".csv"}


def rows():
    if not CSV.exists():
        return []
    with CSV.open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream))


def latest(value):
    return re.split(r"\n\[[^\]\n]+\] ", value)[-1].split("] ", 1)[-1]


def verified(row):
    return row["status"] == "passed" and bool(re.match(r"^passed(?:\b|:)", latest(row["retest"]), re.I))


def import_json(path):
    updates = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(updates, list):
        raise ValueError("Expected a JSON array of objects")
    for item in updates:
        if not isinstance(item, dict) or not re.fullmatch(r"(?:MD|MER|CLI)-\d{3,}", str(item.get("id", ""))):
            raise ValueError("Every object needs an id such as MD-001, MER-001 or CLI-001")
        if set(item) - set(FIELDS):
            raise ValueError(f"Unknown columns: {set(item) - set(FIELDS)}")
        if any(not isinstance(v, str) for v in item.values()):
            raise ValueError("All column values must be strings")
        if "status" in item and item["status"] not in STATUSES:
            raise ValueError(f"status must be one of {STATUSES}")
    AUDIT.mkdir(parents=True, exist_ok=True)
    CSV.parent.mkdir(parents=True, exist_ok=True)
    with (AUDIT / "import.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        current = {r["id"]: r for r in rows()}
        stamp = datetime.now(timezone.utc).isoformat(timespec="microseconds")
        for item in updates:
            row = current.setdefault(item["id"], dict.fromkeys(FIELDS, ""))
            old_status = row["status"]
            for key, value in item.items():
                if key in HISTORY:
                    if value and (key == "retest" or value != latest(row[key])):
                        row[key] = (row[key] + "\n" if row[key] else "") + f"[{stamp}] {value}"
                elif key != "updated_at":
                    row[key] = value
            row["status"] = row["status"] or "inventoried"
            if old_status and old_status != row["status"] and not item.get("retest") and row["retest"]:
                row["retest"] += f"\n[{stamp}] pending: status changed to {row['status']}; record a fresh retest"
            row["updated_at"] = stamp
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(mode="w", newline="", encoding="utf-8", dir=CSV.parent, delete=False) as stream:
                temporary = stream.name
                writer = csv.DictWriter(stream, fieldnames=FIELDS)
                writer.writeheader()
                writer.writerows(current.values())
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, CSV)
        finally:
            if temporary and os.path.exists(temporary):
                os.unlink(temporary)
    print(f"Imported {len(updates)} updates; {len(current)} stories in {CSV}")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        path = unquote(urlsplit(self.path).path)
        if path == "/api/audit":
            records = rows()
            tour = SHOWCASE / "markie-product-tour.mp4"
            tour_exists = tour.is_file() and tour.resolve().is_relative_to(SHOWCASE.resolve())
            selected_verified = sum(verified(r) and bool(r["showcase"].strip()) and r["showcase"].strip().lower() not in {"no", "false", "none", "0"} for r in records)
            videos = [{"name": tour.name, "url": "/artifacts/feature-audit/showcase/" + quote(tour.name)}] if tour_exists and selected_verified else []
            report = ROOT / "docs/feature-audit-report.md"
            report_text = report.read_text(encoding="utf-8") if report.is_file() else ""
            payload = {"rows": records, "verified": sum(verified(r) for r in records), "statuses": STATUSES,
                       "deliverables": {"videos": videos, "showcase_tour_exists": tour_exists,
                                        "showcase_selected_verified": selected_verified, "report_exists": bool(report_text),
                                        "report_final": bool(re.search(r"^Status:\s*complete[d]?\b", report_text, re.I | re.M))}}
            self.send_data(json.dumps(payload).encode(), "application/json")
            return
        routes = {"/": ASSETS / "index.html", "/style.css": ASSETS / "style.css", "/app.js": ASSETS / "app.js", "/download/feature-audit.csv": CSV,
                  "/download/feature-audit-report.md": ROOT / "docs/feature-audit-report.md"}
        target = routes.get(path)
        attachment = path.startswith("/download/")
        if path.startswith("/artifacts/"):
            relative = Path(path.removeprefix("/artifacts/"))
            base = AUDIT
            allowed = SAFE
            parts = relative.parts
            if len(parts) >= 3 and parts[0] == "feature-audit" and parts[1] in {"markdown", "mermaid", "cli", "showcase"}:
                base = ROOT / ".amp/in/artifacts/feature-audit" / parts[1]
                relative = Path(*parts[2:])
                if parts[1] == "showcase":
                    allowed = {".mp4", ".png"}
            base = base.resolve()
            candidate = (base / relative).resolve()
            # Only audit evidence files; no directory listings, symlinks escaping the root,
            # dotfiles, scripts, HTML or arbitrary repository access.
            if candidate.is_relative_to(base) and all(not p.startswith(".") for p in relative.parts) and candidate.suffix.lower() in allowed:
                target = candidate
                attachment = candidate.suffix.lower() in {".svg", ".pdf"}
        if not target or not target.is_file():
            self.send_error(404)
            return
        mime = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
        if target.suffix in {".md", ".log"}:
            mime = "text/plain"
        if target.suffix.lower() == ".mp4":
            mime = "video/mp4"
        with target.open("rb") as stream:
            self.send_headers(mime, os.fstat(stream.fileno()).st_size, attachment)
            shutil.copyfileobj(stream, self.wfile, length=64 * 1024)

    def send_data(self, data, mime, attachment=False):
        self.send_headers(mime, len(data), attachment)
        self.wfile.write(data)

    def send_headers(self, mime, length, attachment=False):
        self.send_response(200)
        self.send_header("Content-Type", mime + ("; charset=utf-8" if mime.startswith("text/") else ""))
        self.send_header("Content-Length", str(length))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Security-Policy", "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; media-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'")
        if attachment:
            self.send_header("Content-Disposition", "attachment")
        self.end_headers()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    importer = commands.add_parser("import", help="Merge a JSON array into the canonical CSV by id")
    importer.add_argument("path")
    server = commands.add_parser("serve", help="Serve the read-only audit on all interfaces")
    server.add_argument("--port", type=int, default=8766)
    args = parser.parse_args()
    if args.command == "import":
        try:
            import_json(args.path)
        except (ValueError, OSError) as error:
            parser.exit(1, f"Import failed: {error}\n")
    else:
        server = ThreadingHTTPServer(("0.0.0.0", args.port), Handler)
        print(f"Feature audit listening on 0.0.0.0:{server.server_port}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
