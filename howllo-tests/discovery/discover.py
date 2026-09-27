"""Discover testable Howllo capabilities from the checked-out source tree.

This intentionally reports evidence, not test results. In particular, an API
route is not proof that its authorization or behavior is correct.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


HTTP_METHODS = {"get", "post", "put", "patch", "delete"}
ROUTE = re.compile(
    r'#\[(?:actix_web::)?(get|post|put|patch|delete)\("([^"\n]+)"\)\]\s*'
    r'(?:pub\s+)?async\s+fn\s+(\w+)',
    re.MULTILINE,
)

# These are the behavior probes required by the regression suite. Endpoints
# are looked up in the registered route table, never assumed to exist.
PROBES = {
    "board.create": [("POST", "/api/admin/boards")],
    "board.update": [("PATCH", "/api/admin/boards/{board_id}")],
    "board.delete": [("DELETE", "/api/admin/boards/{board_id}")],
    "post.create": [("POST", "/api/boards/{board_slug}/posts")],
    "post.edit": [("PATCH", "/api/posts/{post_id}")],
    "post.delete_own": [("DELETE", "/api/posts/{post_id}")],
    "post.comment": [("POST", "/api/posts/{post_id}/comments")],
    "post.vote": [("POST", "/api/posts/{post_id}/vote")],
    "post.unvote": [("DELETE", "/api/posts/{post_id}/vote")],
    "post.status_history": [("GET", "/api/posts/{post_id}/status-history")],
    "moderation.review": [("PATCH", "/api/admin/posts/{post_id}/review")],
    "moderation.lock": [("PATCH", "/api/admin/posts/{post_id}/lock")],
    "moderation.duplicate": [("PATCH", "/api/admin/posts/{post_id}/duplicate")],
    "invitation.create": [("POST", "/api/admin/invitations")],
    "invitation.accept": [("POST", "/api/me/invitations/{invitation_id}/accept")],
    "invitation.reject": [("POST", "/api/me/invitations/{invitation_id}/reject")],
    "invitation.revoke": [("POST", "/api/admin/invitations/{invitation_id}/withdraw")],
    "audit.list": [("GET", "/api/admin/audit-logs")],
    "realtime.websocket": [("GET", "/ws")],
    "membership.custom_role_grants": [("PUT", "/api/admin/roles/{role_id}/permissions")],
}


def read(root: Path, relative: str) -> str:
    path = root / relative
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def source_routes(root: Path) -> list[dict]:
    startup = read(root, "howllo-server/src/startup/mod.rs")
    registered = set(re.findall(r"\.service\(([\w:]+)\)", startup))
    routes = []
    for path in sorted((root / "howllo-server/src").rglob("*.rs")):
        source = path.read_text(encoding="utf-8")
        module = path.relative_to(root / "howllo-server/src").with_suffix("")
        parts = module.parts[:-1] if module.name == "mod" else module.parts
        prefix = "::".join(parts)
        for method, url, function in ROUTE.findall(source):
            symbol = f"{prefix}::{function}" if prefix else function
            if symbol in registered or function in registered:
                routes.append({"method": method.upper(), "path": url,
                               "symbol": symbol, "source": str(path.relative_to(root))})
    return sorted(routes, key=lambda item: (item["path"], item["method"], item["symbol"]))


def openapi_routes(source: str) -> list[dict]:
    """Read only path/method headings; no YAML dependency or schema inference."""
    paths: set[tuple[str, str]] = set()
    in_paths = False
    path = None
    for line in source.splitlines():
        if line == "paths:":
            in_paths = True
            continue
        if in_paths and line and not line.startswith(" "):
            break
        heading = re.fullmatch(r"  (/[^:]+):\s*", line)
        if heading:
            path = "/api" + heading.group(1)
        method = re.fullmatch(r"    (get|post|put|patch|delete):\s*", line)
        if path and method:
            paths.add((method.group(1).upper(), path))
    return [{"method": method, "path": path} for method, path in sorted(paths)]


def string_array(source: str, constant: str) -> list[str]:
    match = re.search(rf"\b{re.escape(constant)}\b[^=]*=\s*\[([^]]*)\]", source, re.S)
    return re.findall(r'"([^"\n]+)"', match.group(1)) if match else []


def board_types(root: Path) -> dict:
    frontend = read(root, "howllo-web/lib/board-experience.ts")
    match = re.search(r"\bBOARD_PRESETS\b[^=]*=\s*\[(.*?)\];", frontend, re.S)
    presets = []
    if match:
        for obj in re.findall(r"\{([^{}]+)\}", match.group(1)):
            value = re.search(r'value:\s*"([^"]+)"', obj)
            votes = re.search(r"defaultVotes:\s*(true|false)", obj)
            comments = re.search(r"defaultComments:\s*(true|false)", obj)
            if value:
                presets.append({"type": value.group(1),
                                "default_votes": votes.group(1) == "true" if votes else None,
                                "default_comments": comments.group(1) == "true" if comments else None})
    migration_dir = root / "howllo-server/db/migrations"
    schema = read(root, "howllo-server/db/migrations/20240101000000_init.sql")
    all_migrations = "\n".join(path.read_text(encoding="utf-8") for path in sorted(migration_dir.glob("*.sql")))
    backend = read(root, "howllo-server/src/dto/mod.rs")
    free_form = bool(re.search(r"board_type\s+VARCHAR\([^)]*\)\s+NOT NULL", schema)) and \
        not bool(re.search(r"CHECK\s*\(\s*board_type\s+IN", all_migrations, re.I))
    return {
        "frontend_presets": presets,
        "database_accepts_free_form_type": free_form,
        "backend_requires_nonempty_type": 'board_type is required' in backend,
        "source": "howllo-web/lib/board-experience.ts",
        "schema_source": "howllo-server/db/migrations/20240101000000_init.sql",
    }


def role_policy(root: Path) -> dict:
    roles = string_array(read(root, "howllo-server/src/domain/role.rs"), "ALL_MEMBERSHIP_ROLES")
    permissions_source = read(root, "howllo-server/src/domain/permission.rs")
    enum = re.search(r"pub enum Permission\s*\{([^}]*)\}", permissions_source, re.S)
    permissions = re.findall(r"^\s*(\w+)\s*,", enum.group(1), re.M) if enum else []
    policy = read(root, "howllo-server/src/auth/context.rs")
    grant_body = re.search(r"fn has_permission\(.*?match role\s*\{(.*?)\n\s*\}\n\}", policy, re.S)
    grants = {}
    if grant_body:
        body = grant_body.group(1)
        for role in roles:
            arm = re.search(rf"Role::{role.title()}\s*=>\s*(true|matches!\()(.*?)(?=\n\s*Role::|\Z)", body, re.S)
            if arm:
                grants[role] = permissions if arm.group(1) == "true" else [
                    name for name in permissions if re.search(rf"\b{re.escape(name)}\b", arm.group(2))
                ]
    return {"roles": roles, "permissions": permissions, "grants": grants,
            "source": "howllo-server/src/auth/context.rs"}


def discover(root: Path) -> dict:
    root = root.resolve()
    routes = source_routes(root)
    documented = openapi_routes(read(root, "howllo-server/openapi.yaml"))
    actual_set = {(r["method"], r["path"]) for r in routes}
    docs_set = {(r["method"], r["path"]) for r in documented}
    probes = {}
    for name, expected in sorted(PROBES.items()):
        present = all(item in actual_set for item in expected)
        probes[name] = {
            "status": "AVAILABLE" if present else "MISSING_CAPABILITY",
            "required_routes": [{"method": method, "path": path} for method, path in expected],
            "missing_routes": [{"method": method, "path": path} for method, path in expected if (method, path) not in actual_set],
        }
    statuses = string_array(read(root, "howllo-server/src/domain/status.rs"), "ALL_FEEDBACK_STATUSES")
    schema = read(root, "howllo-server/db/migrations/20240101000000_init.sql")
    status_constraint = re.search(r"CONSTRAINT\s+posts_status_check\s+CHECK\s*\(status\s+IN\s*\(([^)]*)\)", schema, re.I)
    db_statuses = re.findall(r"'([^']+)'", status_constraint.group(1)) if status_constraint else []
    pill = read(root, "howllo-web/components/status-pill.tsx")
    labels = re.search(r"const LABELS[^=]*=\s*\{([^}]*)\}", pill, re.S)
    ui_statuses = re.findall(r'^\s*(?:"([^"]+)"|(\w+))\s*:', labels.group(1), re.M) if labels else []
    ui_statuses = [quoted or bare for quoted, bare in ui_statuses]
    web_pages = sorted(str(path.relative_to(root / "howllo-web/app")) for path in
                       (root / "howllo-web/app").rglob("page.tsx"))
    app_pages = sorted(str(path.relative_to(root / "howllo-app/app")) for path in
                       (root / "howllo-app/app").rglob("page.tsx"))
    roles = role_policy(root)
    personas = {name: {"status": "AVAILABLE" if name in roles["roles"] else "MISSING_CAPABILITY"}
                for name in ("owner", "admin", "moderator", "member", "support", "limited_staff", "viewer")}
    board_schema = read(root, "howllo-server/db/migrations/20240101000000_init.sql")
    visibility = ["public", "private"] if re.search(r"is_private\s+BOOLEAN", board_schema) else []
    if re.search(r"\bvisibility\b[^\n]*\b(authenticated|members)\b", board_schema, re.I):
        visibility.append("authenticated")
    probes["board.visibility.authenticated"] = {
        "status": "AVAILABLE" if "authenticated" in visibility else "MISSING_CAPABILITY",
        "required_schema_state": "authenticated",
    }
    return {
        "schema_version": 1,
        "source_root": str(root),
        "api": {"registered_routes": routes, "openapi_routes": documented,
                "registered_undocumented": [{"method": m, "path": p} for m, p in sorted(actual_set - docs_set)],
                "documented_unregistered": [{"method": m, "path": p} for m, p in sorted(docs_set - actual_set)]},
        "board_types": board_types(root),
        "board_visibilities": visibility,
        "post_statuses": {"backend": statuses, "database": db_statuses,
                          "frontend_labels": ui_statuses,
                          "consistent": bool(statuses) and set(statuses) == set(db_statuses),
                          "missing_frontend_labels": sorted(set(statuses) - set(ui_statuses))},
        "role_policy": roles,
        "persona_capabilities": personas,
        "frontend": {"customer_pages": web_pages, "staff_pages": app_pages},
        "capabilities": probes,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Discover Howllo test capabilities")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, help="Write manifest to this path; stdout by default")
    parser.add_argument("--json", action="store_true", help="JSON is the default output; accepted for CLI callers")
    args = parser.parse_args()
    manifest = json.dumps(discover(args.root), indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(manifest, encoding="utf-8")
    else:
        print(manifest, end="")


if __name__ == "__main__":
    main()
