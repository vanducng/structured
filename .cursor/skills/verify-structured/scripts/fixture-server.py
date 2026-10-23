#!/usr/bin/env python3
"""Local stand-in for the Structured MCP server. Serves generic tasks dated around today and logs every tool call."""
import datetime as dt
import json
import os
import sys
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from zoneinfo import ZoneInfo

HOME_NAME = os.environ.get("TZ") or os.path.realpath("/etc/localtime").partition("zoneinfo/")[2] or "UTC"
HOME = ZoneInfo(HOME_NAME)
TODAY = dt.datetime.now(HOME).date()
LOG = None


def day(offset=0):
    return (TODAY + dt.timedelta(days=offset)).isoformat()


def stamp():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def sub(title, done=False):
    return {"id": str(uuid.uuid4()), "title": title, "completed_at": stamp() if done else None}


def task(title, start=None, minutes=0, offset=0, inbox=False, done=False, color="day", **extra):
    base = {
        "id": str(uuid.uuid4()),
        "title": title,
        "note": "",
        "day": None if inbox else day(offset),
        "start_time": start,
        "duration": minutes,
        "is_all_day": not inbox and start is None,
        "is_in_inbox": inbox,
        "completed_at": stamp() if done else None,
        "energy_level": 0,
        "color": color,
        "symbol": "text.badge.checkmark",
        "subtasks": [],
        "order_index": None,
        "external_calendar_id": None,
        "timezone": HOME_NAME if start is not None else None,
    }
    base.update(extra)
    return base


def recurring(title, **kwargs):
    t = task(title, **kwargs)
    t["recurring_id"] = str(uuid.uuid4())
    t["is_recurring"] = True
    t["timezone"] = None
    return t


TASKS = [
    recurring("Go for a run", done=True, color="#ED732E", energy_level=3),
    recurring("Read 20 pages", color="forest"),
    recurring("Rise and shine", start=8.0, done=True),
    task("Team sync", 8.5, 45, color="#f09343", note="Agenda lives in the shared doc.", external_calendar_id="cal-1"),
    task("Review the API pull request", 9.0, 30, color="night"),
    task("Ship the release notes", 11.25, 60, done=True),
    task(
        "Finish the billing export (ENG-3442)",
        13.0,
        210,
        color="dawn",
        energy_level=2,
        note="Export runs nightly. Check the totals against last month before switching the job on.",
        subtasks=[sub("Map the fields", True), sub("Write the backfill"), sub("Dry run on staging")],
    ),
    task("Read the hiring brief", 16.75, 45),
    task("Triage open tickets", 17.75, 75, subtasks=[sub("Billing", True), sub("Search", True)]),
    task("Data team huddle", 20.0, 45, color="#f09343"),
    task("Retention review", 21.5, 30, color="#f09343"),
    task("Standup with the US team", 20.5, 30, offset=-1, done=True, color="#f09343"),
    task("Pair on the export job", 21.25, 90, offset=-1, color="night"),
    task("Send the weekly update", offset=-1, color="twilight"),
    task("Plan the week", 9.0, 60, offset=1, color="night"),
    task("Dentist", 15.0, 45, offset=1, color="sunshine"),
    task("Try the new espresso grinder", inbox=True, color="sunshine"),
    task("Draft the onboarding checklist", inbox=True, subtasks=[sub("Accounts"), sub("Laptop setup")]),
    task("Renew the domain", inbox=True, color="twilight"),
    task("Write up the search experiment", inbox=True, color="night"),
    task("Book flights for the offsite", inbox=True),
    task("Sort the photo backlog", inbox=True, done=True),
    task("Cancel the unused trial", inbox=True, done=True),
]


def zone(args):
    try:
        return ZoneInfo(args["timezone"])
    except (KeyError, ValueError):
        return HOME


def seen(t, tz):
    """Like the real server: the day never moves; a zoned time counts from that day's midnight in the asking zone."""
    if t["start_time"] is None or not t["day"] or t["is_all_day"] or not t.get("timezone"):
        return t
    on = dt.date.fromisoformat(t["day"])
    at = dt.datetime.combine(on, dt.time(), ZoneInfo(t["timezone"])) + dt.timedelta(hours=t["start_time"])
    hours = (at - dt.datetime.combine(on, dt.time(), tz)).total_seconds() / 3600
    return {**t, "start_time": round(hours, 6)}


def find(args):
    if args.get("recurring_id"):
        return next((t for t in TASKS if t.get("recurring_id") == args["recurring_id"] and t["day"] == args.get("day")), None)
    # Like the real server, a recurring day is only addressable by recurring_id + day.
    return next((t for t in TASKS if t["id"] == args.get("id") and not t.get("is_recurring")), None)


def ordered(tasks):
    return sorted(tasks, key=lambda t: (t["start_time"] is not None, t["start_time"] or 0))


def call(name, args):
    start = args.get("start_time")
    if start is not None and not -12 <= start <= 36:
        raise ValueError("start_time must be between -12 and 36")
    if name == "get_account":
        return {"email": "fixture@example.com", "plan": "pro"}
    tz = zone(args)
    if name in ("get_today", "get_tasks_for_day"):
        on = args.get("day", dt.datetime.now(tz).date().isoformat())
        return {"view": "today", "day": on, "tasks": ordered([seen(t, tz) for t in TASKS if t["day"] == on])}
    if name == "get_inbox":
        return {"view": "inbox", "tasks": [t for t in TASKS if t["is_in_inbox"]]}
    if name == "list_tasks":
        lo, hi = args.get("day_from", day()), args.get("day_to", "9999-12-31")
        hits = [seen(t, tz) for t in TASKS if t["day"] and lo <= t["day"] <= hi]
        if "is_completed" in args:
            hits = [t for t in hits if bool(t["completed_at"]) == args["is_completed"]]
        if args.get("title_contains"):
            hits = [t for t in hits if args["title_contains"].lower() in t["title"].lower()]
        return {"tasks": ordered(hits)[: args.get("limit", 50)]}
    if name == "create_task":
        inbox = "day" not in args
        t = task(args["title"], args.get("start_time"), args.get("duration", 0), inbox=inbox, note=args.get("note", ""))
        if not inbox:
            t["day"] = args["day"]
            if t["start_time"] is not None:
                t["timezone"] = args.get("timezone", HOME_NAME)
        t["subtasks"] = [sub(s["title"]) for s in args.get("subtasks", [])]
        TASKS.append(t)
        return {"task": seen(t, tz)}
    t = find(args)
    if t is None:
        raise LookupError("Task not found")
    if name == "complete_task":
        t["completed_at"] = stamp()
    elif name == "delete_task":
        TASKS.remove(t)
        return {"deleted": True}
    elif name == "update_task":
        for key in ("title", "note", "start_time", "duration", "is_all_day", "energy_level", "color"):
            if key in args:
                t[key] = args[key]
        if "is_completed" in args:
            t["completed_at"] = stamp() if args["is_completed"] else None
        if args.get("is_in_inbox"):
            t.update(is_in_inbox=True, day=None, start_time=None)
        target = args.get("new_day") or (None if args.get("recurring_id") else args.get("day"))
        if target:
            t.update(day=target, is_in_inbox=False)
        if "timezone" in args and t["start_time"] is not None:
            t["timezone"] = args["timezone"]
        if args.get("update_action") == "only_this" and t.get("is_recurring"):
            t.update(id=str(uuid.uuid4()), is_recurring=False)
            t.pop("recurring_id", None)
        if "subtasks" in args:
            t["subtasks"] = [
                {"id": s.get("id") or str(uuid.uuid4()), "title": s["title"], "completed_at": s.get("completed_at")}
                for s in args["subtasks"]
            ]
        return {"target": "task", "operation": "updated", "task": seen(t, tz)}
    return {"task": seen(t, tz)}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if "id" not in body:
            self.send_response(202)
            self.end_headers()
            return
        if body["method"] == "initialize":
            result = {"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "fixture"}}
        else:
            name, args = body["params"]["name"], body["params"].get("arguments", {})
            with open(LOG, "a") as log:
                log.write(json.dumps({"tool": name, "arguments": args}) + "\n")
            try:
                data = call(name, args)
                result = {"content": [{"type": "text", "text": json.dumps(data)}], "structuredContent": data}
            except Exception as err:  # noqa: BLE001 - the error text is the tool result
                result = {"isError": True, "content": [{"type": "text", "text": str(err)}]}
        payload = json.dumps({"jsonrpc": "2.0", "id": body["id"], "result": result}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


if __name__ == "__main__":
    port_file, LOG = sys.argv[1], sys.argv[2]
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    with open(port_file, "w") as handle:
        handle.write(str(server.server_address[1]))
    server.serve_forever()
