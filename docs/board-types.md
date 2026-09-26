# Board experiences

Each workspace can contain several boards. A board's type controls its public layout and the post flow. The roadmap is a separate workspace page built from posts with progress statuses.

| Type | Public layout | Who starts a post | Default interaction |
| --- | --- | --- | --- |
| Feature requests | Ranked request list with vote counts and progress status | Workspace participants | Vote and comment |
| Bug reports | Issue cards with status; report form asks for reproduction steps, expected result, and actual result | Workspace participants | Comment; affected reaction is optional |
| Discussions | Conversation list with reply counts | Workspace participants | Reply; likes are optional |
| Announcements | Dated update timeline | Workspace staff through the Staff App | Respond; helpful reaction is optional |

Staff can edit each board's introduction, background color, icon, header image, background image, categories, and vote/comment switches. Images are uploaded through workspace storage and fitted to a maximum of 1800 × 600 for headers or 2000 × 1400 for backgrounds. Categories belong to one board; tags are shared by the workspace. A post can have one category and up to three tags. The board page supports searching post title/body and filtering by category or tag.

The API enforces the interaction switches and rejects visitor posts on announcement boards. Boards start as drafts and remain invisible until both the board and workspace are published. Removing a category leaves its posts intact and uncategorized.

Existing board types are mapped for display: `feedback` to Feature requests, `support` to Bug reports, `general` and `internal` to Discussions, and `changelog` and `updates` to Announcements. Existing boards keep their current vote and comment settings until staff changes them.
