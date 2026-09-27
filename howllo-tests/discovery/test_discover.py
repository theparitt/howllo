"""Run with: python3 -m unittest discover -s howllo-tests/discovery -v"""

import tempfile
import unittest
from pathlib import Path

from discover import discover, openapi_routes, source_routes


def write(root: Path, relative: str, content: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class DiscoveryTests(unittest.TestCase):
    def test_real_source_discovers_registered_routes_and_policy(self):
        root = Path(__file__).resolve().parents[2]
        result = discover(root)
        actual = {(item["method"], item["path"]) for item in result["api"]["registered_routes"]}
        self.assertIn(("POST", "/api/admin/boards"), actual)
        self.assertIn(("POST", "/api/me/invitations/{invitation_id}/reject"), actual)
        self.assertIn(("GET", "/ws"), actual)
        self.assertEqual(result["capabilities"]["invitation.reject"]["status"], "AVAILABLE")
        self.assertEqual(result["capabilities"]["post.delete_own"]["status"], "MISSING_CAPABILITY")
        self.assertEqual(result["capabilities"]["board.visibility.authenticated"]["status"], "MISSING_CAPABILITY")
        self.assertEqual(result["persona_capabilities"]["support"]["status"], "MISSING_CAPABILITY")
        self.assertIn("moderator", result["role_policy"]["roles"])
        self.assertNotIn("ManageBoards", result["role_policy"]["grants"]["moderator"])
        self.assertTrue(result["post_statuses"]["consistent"])
        self.assertIn("announcements", result["board_types"]["staff_only_post_types"])

    def test_fifth_type_is_discovered_without_code_change(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root, "howllo-web/lib/board-experience.ts", '''
export const BOARD_PRESETS = [
  { value: "feature-requests", defaultVotes: true, defaultComments: true },
  { value: "questions", defaultVotes: false, defaultComments: true },
];
''')
            write(root, "howllo-server/db/migrations/20240101000000_init.sql",
                  "CREATE TABLE boards (board_type VARCHAR(50) NOT NULL);")
            write(root, "howllo-server/src/dto/mod.rs", '"board_type is required"')
            result = discover(root)
            self.assertEqual([item["type"] for item in result["board_types"]["frontend_presets"]],
                             ["feature-requests", "questions"])
            self.assertTrue(result["board_types"]["database_accepts_free_form_type"])

    def test_unregistered_route_is_not_available_and_openapi_drift_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root, "howllo-server/src/startup/mod.rs", "cfg.service(boards::api::list_boards);")
            write(root, "howllo-server/src/boards/api.rs", '''
#[get("/api/boards")]
pub async fn list_boards() {}
#[delete("/api/posts/{post_id}")]
pub async fn delete_post() {}
''')
            write(root, "howllo-server/openapi.yaml", '''
openapi: 3.0.3
paths:
  /boards:
    get:
      summary: List boards
  /posts/{post_id}:
    delete:
      summary: Delete post
''')
            result = discover(root)
            self.assertEqual(len(source_routes(root)), 1)
            self.assertEqual(result["capabilities"]["post.delete_own"]["status"], "MISSING_CAPABILITY")
            self.assertEqual(result["api"]["documented_unregistered"],
                             [{"method": "DELETE", "path": "/api/posts/{post_id}"}])

    def test_openapi_reader_stops_before_components(self):
        source = '''
paths:
  /boards:
    get:
      summary: List
components:
  /not-a-route:
    post:
'''.strip()
        self.assertEqual(openapi_routes(source), [{"method": "GET", "path": "/api/boards"}])


if __name__ == "__main__":
    unittest.main()
