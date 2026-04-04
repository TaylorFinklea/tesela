# Tesela Type System

Tesela treats types as content, not hardcoded classes. A Tag is a page. A Property is a page. The SQLite index caches those pages so the server and UI can resolve schemas quickly.

## Core idea: everything is a page
| Kind | Stored as | Indexed into | Purpose |
| --- | --- | --- | --- |
| Tag | A normal note whose frontmatter has `type: "Tag"` | `tag_defs` | Defines a reusable type such as `Task` or `Project` |
| Property | A normal note whose frontmatter has `type: "Property"` | `property_defs` | Defines a reusable field such as `Status` or `Deadline` |
| Block value | A bullet block inside any note | `block_properties` | Stores actual `status:: todo` style values for querying |

Example Tag page:
```yaml
---
title: "Task"
type: "Tag"
extends: "Root Tag"
icon: "☑"
tag_properties: ["Status", "Priority", "Deadline", "Scheduled"]
---
```

Example Property page:
```yaml
---
title: "Status"
type: "Property"
value_type: "select"
choices: ["backlog", "todo", "doing", "done"]
default: "todo"
---
```

## Tag pages
Tag pages define a named type and the property schema that should be available to pages or blocks using that tag.

| Field | Meaning |
| --- | --- |
| `type: "Tag"` | Marks the note as a tag definition |
| `extends` | Optional parent tag; used for inheritance |
| `tag_properties` | Ordered list of property page titles attached to the tag |
| `icon` | Display icon cached into `tag_defs.icon` |
| `color` | Optional display color cached into `tag_defs.color` |

When the server indexes a Tag page, it stores the tag name, parent, icon, color, and `tag_properties` JSON in `tag_defs`.

## Property pages
Property pages define field schemas that tag pages can reference by name.

| Field | Meaning |
| --- | --- |
| `type: "Property"` | Marks the note as a property definition |
| `value_type` | Logical value type such as `text`, `select`, `date`, or `node` |
| `choices` | Allowed values for select-like properties |
| `default` | Default value when one exists |
| `multiple_values` | Optional boolean for multi-value properties |
| `hide_empty` | Optional UI hint |
| `description` | Optional human-readable help text |

When indexed, these fields are cached in `property_defs`.

## Inheritance through `extends`
Tesela resolves a tag by walking the `extends` chain from child to parent until it reaches the root.

Example:
```yaml
Task -> Root Tag
LifeProject -> Root Tag
```

Resolution behavior:
| Step | Result |
| --- | --- |
| Read the child tag from `tag_defs` | Start with the child tag’s icon and color |
| Follow `extends` up the chain | Collect every parent `tag_properties` list |
| Deduplicate property names | Child-listed properties win if repeated |
| Resolve each property name via `property_defs` | Produce full `PropertyDef` schemas for the UI/API |

This is why the server route `GET /types/{name}` returns resolved property schemas, not just raw property names.

## Built-in tag pages
On server startup, Tesela writes built-in pages into `notes/` if they do not already exist.

| Built-in tag | Parent | Properties |
| --- | --- | --- |
| `Root Tag` | None | none |
| `Task` | `Root Tag` | `Status`, `Priority`, `Deadline`, `Scheduled` |
| `Project` | `Root Tag` | `Status`, `Deadline` |
| `Person` | `Root Tag` | `Email`, `Team` |
| `Domain` | `Root Tag` | `Description` |
| `LifeProject` | `Root Tag` | `Status`, `DomainRef`, `Deadline`, `Description` |
| `Issue` | `Root Tag` | `IssueStatus`, `DomainRef`, `Description` |
| `Ritual` | `Root Tag` | `Cadence`, `DomainRef` |
| `ScheduledItem` | `Root Tag` | `Cadence`, `DomainRef`, `LastCompleted` |

## Built-in property pages
| Built-in property | Type | Notable values/default |
| --- | --- | --- |
| `Status` | `select` | backlog, todo, doing, in-review, done, canceled; default `todo` |
| `Priority` | `select` | critical, high, medium, low; default `medium` |
| `Deadline` | `date` | none |
| `Scheduled` | `date` | none |
| `Email` | text schema via fallback only in older registry; built-in page is not auto-seeded |
| `Team` | text schema via fallback only in older registry; built-in page is not auto-seeded |
| `IssueStatus` | `select` | inbox, open, thinking, resolved, became-project, became-task; default `inbox` |
| `Cadence` | `select` | daily, weekly, biweekly, monthly, quarterly, yearly |
| `Description` | `text` | none |
| `LastCompleted` | `date` | none |
| `DomainRef` | `node` | links an item to a Domain page |

Note: `Email` and `Team` exist in the older `TypeRegistry` fallback used when no DB-backed tag data is available, but the current server bootstrap only auto-creates the property pages listed above.

## How blocks inherit properties from tags
A block declares a tag inline, such as:
```md
- Fix search ranking #Task
  status:: doing
  priority:: high
```

What happens:
| Layer | Behavior |
| --- | --- |
| Block parser | Extracts `#Task` into `tags` and `status:: doing` into block `properties` |
| Tag definition | Says a `Task` block is expected to support `Status`, `Priority`, `Deadline`, `Scheduled` |
| Property definitions | Describe the allowed shape of those values |
| Block index | Stores the actual values in `block_properties` for filtering and sorting |

So blocks inherit the schema from their tags, but they only get concrete values when the block actually contains property lines. The schema comes from Tag and Property pages; the data comes from the block itself.
