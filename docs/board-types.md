# Board experiences

Each workspace can contain several boards. A board's type controls its public layout and the post flow. The roadmap is a separate workspace page built from posts with progress statuses.

| Type | Public layout | Who starts a post | Default interaction |
| --- | --- | --- | --- |
| Feature requests | Ranked request list with vote counts and progress status | Workspace participants | Vote and comment |
| Bug reports | Issue cards with status; report form asks for reproduction steps, expected result, and actual result | Workspace participants | Comment; affected reaction is optional |
| Discussions | Conversation list with reply counts | Workspace participants | Reply; likes are optional |
| Announcements | Dated update timeline | Workspace staff through the Staff App | Respond; helpful reaction is optional |

Staff can edit each board's introduction, background color, icon, and vote/comment switches. The API enforces the interaction switches and rejects visitor posts on announcement boards. Boards start as drafts and remain invisible until both the board and workspace are published.

Existing board types are mapped for display: `feedback` to Feature requests, `support` to Bug reports, `general` and `internal` to Discussions, and `changelog` and `updates` to Announcements. Existing boards keep their current vote and comment settings until staff changes them.
