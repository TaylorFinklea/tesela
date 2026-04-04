# Tesela MCP Tools

Tesela MCP exposes six note-centric tools. Each successful call returns an MCP result object with a `content` array; Tesela currently responds with a single text item whose `text` field contains either formatted note text or pretty-printed JSON.

| Tool | Description | Parameters | Returns | Example usage scenario |
| --- | --- | --- | --- | --- |
| `search_notes` | Full-text search through notes. | `query: string` required; `limit: integer` optional, defaults to `10`. | `content[0].text` is a JSON array of search matches: `{ "id": string, "title": string, "snippet": string, "tags": string[] }`. | An assistant needs the top 5 notes mentioning `"deadline"` before drafting a weekly planning summary. |
| `get_note` | Get a note by ID or by fuzzy title lookup. | `id: string` optional; `title: string` optional. One of them must be provided. | On success, `content[0].text` is formatted markdown-like text: heading with the note title, then `ID`, `Tags`, and the note body. If no note is found, returns `isError: true` with `content[0].text = "Note not found"`. | A user asks, "Show me the note for `project-alpha`," or "Open the note titled Sprint Plan." |
| `create_note` | Create a new note and index it. | `title: string` required; `content: string` optional, defaults to empty; `tags: string[]` optional. | `content[0].text` is a confirmation string: `Created note 'Title' with ID: note-id`. | An assistant captures a meeting outcome into a new note tagged `Project` and `Decision`. |
| `list_notes` | List notes with optional pagination and tag filtering. | `tag: string` optional; `limit: integer` optional, defaults to `20`; `offset: integer` optional, defaults to `0`. | `content[0].text` is a JSON array of summaries: `{ "id": string, "title": string, "tags": string[], "created": RFC3339 string, "modified": RFC3339 string }`. | A client wants the first 20 notes tagged `Task`, or needs page 2 of all notes for a picker UI. |
| `get_backlinks` | Get all notes that link to a given note. | `id: string` required. | `content[0].text` is a JSON array of backlink summaries: `{ "source": string, "text": string }`, where `source` is populated from the link target field returned by the index and `text` is the source line containing the link. | An assistant is checking which notes reference `roadmap` before renaming or deleting it. |
| `get_daily_note` | Get or create the daily note for a given date. | `date: string` optional in `YYYY-MM-DD` format; omitted means today. | `content[0].text` is formatted text with the note title, `ID`, `Path`, and note body. Invalid dates return an error string. | A daily workflow opens today’s note automatically, or fetches `2026-04-01` during a retrospective. |

## Parameter notes
| Parameter | Type | Required | Used by |
| --- | --- | --- | --- |
| `query` | `string` | Yes | `search_notes` |
| `limit` | `integer` | No | `search_notes`, `list_notes` |
| `id` | `string` | Yes for `get_backlinks`; conditional for `get_note` | `get_note`, `get_backlinks` |
| `title` | `string` | Conditional | `get_note` |
| `content` | `string` | No | `create_note` |
| `tags` | `string[]` | No | `create_note` |
| `tag` | `string` | No | `list_notes` |
| `offset` | `integer` | No | `list_notes` |
| `date` | `string` | No | `get_daily_note` |

## Example call shapes
| Tool | Example arguments |
| --- | --- |
| `search_notes` | `{ "query": "deadline", "limit": 5 }` |
| `get_note` | `{ "id": "project-alpha" }` or `{ "title": "Sprint Plan" }` |
| `create_note` | `{ "title": "Retro Notes", "content": "- Wins\n- Risks", "tags": ["Meeting", "Team"] }` |
| `list_notes` | `{ "tag": "Task", "limit": 20, "offset": 0 }` |
| `get_backlinks` | `{ "id": "roadmap" }` |
| `get_daily_note` | `{ "date": "2026-04-04" }` |
