//! Default Query widgets that ship with every new mosaic — keeps the
//! rail nav (Dailies / Pages / Tasks / Projects / People / Inbox /
//! Calendar / Recent / Pinned) populated from the moment a fresh
//! mosaic is created.
//!
//! Kept in sync with `web/src/lib/system-widgets.ts` — that file
//! continues to auto-create missing widgets on web mount as a safety
//! net (e.g. a user manually deleted one). Server-side seeding here
//! eliminates the race where the rail rendered before the web's
//! idempotent `ensureSystemWidgets` had a chance to run.

use std::fs;
use std::io;
use std::path::Path;

pub struct SystemWidget {
    pub id: &'static str,
    pub title: &'static str,
    pub query: &'static str,
    pub group: Option<&'static str>,
    pub sort: Option<&'static str>,
    pub icon: &'static str,
    pub color: &'static str,
    pub section: &'static str,
}

/// Mirror of `SYSTEM_WIDGETS` in the web client. Source of truth for
/// both initial seeding (server side, runs on mosaic create) and the
/// idempotent ensure pass (web client, runs on every mount).
pub const SYSTEM_WIDGETS: &[SystemWidget] = &[
    SystemWidget {
        id: "dailies",
        title: "Dailies",
        query: "",
        group: None,
        sort: None,
        icon: "calendar",
        color: "amber",
        section: "pinned",
    },
    SystemWidget {
        id: "pages",
        title: "Pages",
        query: "kind:page",
        group: None,
        sort: None,
        icon: "cal",
        color: "amber-2",
        section: "pinned",
    },
    SystemWidget {
        id: "tasks",
        title: "Tasks",
        query: "kind:block tag:Task -status:done",
        group: Some("status"),
        sort: Some("deadline asc"),
        icon: "task",
        color: "rose",
        section: "browse",
    },
    SystemWidget {
        id: "projects",
        title: "Projects",
        query: "kind:page note_type:Project",
        group: None,
        sort: None,
        icon: "project",
        color: "indigo",
        section: "browse",
    },
    SystemWidget {
        id: "people",
        title: "People",
        query: "kind:page note_type:Person",
        group: None,
        sort: None,
        icon: "person",
        color: "plum",
        section: "browse",
    },
    SystemWidget {
        id: "inbox",
        title: "Inbox",
        query: "kind:block -has:status",
        group: None,
        sort: None,
        icon: "inbox",
        color: "teal",
        section: "browse",
    },
    SystemWidget {
        id: "calendar",
        title: "Calendar",
        query: "kind:block has:scheduled",
        group: None,
        sort: Some("scheduled asc"),
        icon: "cal",
        color: "amber-2",
        section: "browse",
    },
    SystemWidget {
        id: "recent",
        title: "Recent",
        query: "kind:page",
        group: None,
        sort: Some("modified desc"),
        icon: "clock",
        color: "ochre",
        section: "saved",
    },
    SystemWidget {
        id: "pinned",
        title: "Pinned",
        query: "kind:page",
        group: None,
        sort: None,
        icon: "pin",
        color: "rose",
        section: "saved",
    },
];

fn render(w: &SystemWidget) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: \"{}\"\n", w.title));
    out.push_str("type: \"Query\"\n");
    out.push_str("tags: []\n");
    out.push_str("---\n");
    out.push_str(&format!("query:: {}\n", w.query));
    if let Some(g) = w.group {
        out.push_str(&format!("group:: {}\n", g));
    }
    if let Some(s) = w.sort {
        out.push_str(&format!("sort:: {}\n", s));
    }
    out.push_str(&format!("icon:: {}\n", w.icon));
    out.push_str(&format!("color:: {}\n", w.color));
    out.push_str(&format!("section:: {}\n", w.section));
    out
}

/// Seed `notes/<id>.md` for every system widget that doesn't already
/// exist. Idempotent — re-running is safe; a user-edited widget is
/// preserved.
pub fn seed(mosaic_root: &Path) -> io::Result<usize> {
    let notes_dir = mosaic_root.join("notes");
    fs::create_dir_all(&notes_dir)?;
    let mut created = 0;
    for w in SYSTEM_WIDGETS {
        let path = notes_dir.join(format!("{}.md", w.id));
        if path.exists() {
            continue;
        }
        fs::write(&path, render(w))?;
        created += 1;
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn seed_creates_all_widgets_first_time() {
        let temp = TempDir::new().unwrap();
        let n = seed(temp.path()).unwrap();
        assert_eq!(n, SYSTEM_WIDGETS.len());
        // Spot-check a couple of widgets landed.
        let dailies = fs::read_to_string(temp.path().join("notes/dailies.md")).unwrap();
        assert!(dailies.contains("title: \"Dailies\""));
        assert!(dailies.contains("section:: pinned"));
        let tasks = fs::read_to_string(temp.path().join("notes/tasks.md")).unwrap();
        assert!(tasks.contains("group:: status"));
    }

    #[test]
    fn seed_is_idempotent_and_preserves_user_edits() {
        let temp = TempDir::new().unwrap();
        seed(temp.path()).unwrap();
        // User edits the dailies widget.
        let dailies_path = temp.path().join("notes/dailies.md");
        fs::write(&dailies_path, "USER EDITED\n").unwrap();
        let n = seed(temp.path()).unwrap();
        assert_eq!(n, 0, "second seed should be a no-op");
        assert_eq!(fs::read_to_string(dailies_path).unwrap(), "USER EDITED\n");
    }
}
