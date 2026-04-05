# Tesela Server API

Base URL examples below assume `http://127.0.0.1:7474`.

## Health
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /health` | None | None | `{ "status": "ok" }` | `curl http://127.0.0.1:7474/health` |

## Notes
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /notes` | `tag?: string`, `limit?: usize` default `100`, `offset?: usize` default `0` | None | `Note[]` where each note includes `id`, `title`, `content`, `body`, `metadata`, `path`, `checksum`, `created_at`, `modified_at`, `attachments` | `curl 'http://127.0.0.1:7474/notes?tag=Task&limit=20&offset=0'` |
| `POST /notes` | None | `{ "title": string, "content": string, "tags"?: string[] }` | `Note` | `curl -X POST http://127.0.0.1:7474/notes -H 'Content-Type: application/json' -d '{"title":"Sprint Plan","content":"---\ntags: [Project]\n---\nBody","tags":["Project"]}'` |
| `GET /notes/daily` | `date?: string` in `YYYY-MM-DD` format | None | `Note` for the requested or current daily note | `curl 'http://127.0.0.1:7474/notes/daily?date=2026-03-30'` |
| `GET /notes/{id}` | None | None | `Note` | `curl http://127.0.0.1:7474/notes/task-123` |
| `PUT /notes/{id}` | None | `{ "content": string }` containing the full note contents, including frontmatter | Updated `Note` re-read from disk after the write | `curl -X PUT http://127.0.0.1:7474/notes/task-123 -H 'Content-Type: application/json' -d '{"content":"---\ntitle: Updated\n---\nNew body"}'` |
| `DELETE /notes/{id}` | None | None | HTTP `204 No Content` | `curl -X DELETE http://127.0.0.1:7474/notes/task-123` |
| `GET /notes/{id}/backlinks` | None | None | `Link[]` with `link_type`, `target`, `text`, `position` | `curl http://127.0.0.1:7474/notes/task-123/backlinks` |
| `GET /notes/{id}/links` | None | None | `Link[]` with `link_type`, `target`, `text`, `position` | `curl http://127.0.0.1:7474/notes/task-123/links` |
| `GET /links` | None | None | `GraphEdge[]` with `source`, `target` | `curl http://127.0.0.1:7474/links` |

## Search
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /search` | `q: string`, `limit?: usize` default `20`, `offset?: usize` default `0` | None | `SearchHit[]` with `note_id`, `title`, `snippet`, `rank`, `tags`, `path` | `curl 'http://127.0.0.1:7474/search?q=deadline&limit=10'` |

## Types
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /types` | None | None | `TypeDefinition[]` with `name`, `description`, `icon`, `color`, `properties` | `curl http://127.0.0.1:7474/types` |
| `GET /types/{name}` | None | None | Resolved `TypeDefinition` for the named type, including inherited properties | `curl http://127.0.0.1:7474/types/Task` |
| `GET /types/{name}/nodes` | None | None | `Note[]` tagged with the requested type | `curl http://127.0.0.1:7474/types/Task/nodes` |
| `GET /types/{name}/blocks` | `filter_property?: string`, `filter_value?: string`, `filters?: string` JSON array of `{ "property": string, "value": string }`, `sort_by?: string`, `sort_dir?: string` (`desc` or ascending by default) | None | `ParsedBlock[]` for blocks tagged with the type; filters use AND logic across all provided property filters | `curl 'http://127.0.0.1:7474/types/Task/blocks?filter_property=status&filter_value=todo&sort_by=priority&sort_dir=desc'` |
| `GET /properties` | None | None | `PropertyDef[]` with `name`, `value_type`, `values`, `default`, `required` | `curl http://127.0.0.1:7474/properties` |

## Tags
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /tags` | None | None | `string[]` of indexed tag names | `curl http://127.0.0.1:7474/tags` |

## WebSocket
| Method + path | Query parameters | Request body | Response shape | Example curl |
| --- | --- | --- | --- | --- |
| `GET /ws` | None | WebSocket upgrade handshake | Server pushes JSON `WsEvent` messages tagged by `event`: `note_created { note: Note }`, `note_updated { note: Note }`, `note_deleted { id: string }` | `curl -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: SGVsbG8sVGVzZWxhIQ==' http://127.0.0.1:7474/ws` |

## Common nested shapes
| Shape | Fields |
| --- | --- |
| `Note.metadata` | `title?: string`, `tags: string[]`, `aliases: string[]`, `note_type?: string`, `custom: object`, `created?: datetime`, `modified?: datetime` |
| `Note.attachments[]` | `id`, `filename`, `mime_type`, `size`, `checksum`, `path`, `note_ids` |
| `TypeDefinition.properties[]` | `name`, `value_type`, `values?: string[]`, `default?: string`, `required: bool` |

Errors are returned as JSON in the form `{ "error": string }`, with `404` for missing resources and `500` for internal failures.
