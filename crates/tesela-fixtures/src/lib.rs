//! Synthetic Tesela mosaic generator for benchmarks + integration
//! tests.
//!
//! Two use cases drive the design:
//!
//! 1. **Performance benchmarks** — generate a 500-2000 note mosaic
//!    that mimics the structural shape of a real Logseq-imported
//!    graph, then run `Indexer::initial_index`, `list_notes`,
//!    `get_typed_blocks_matching`, etc. against it. Phase 14's hot
//!    paths are too tedious to exercise with hand-built fixtures.
//!
//! 2. **Integration-test data** — replace the hand-rolled
//!    `make_fixture_mosaic()` helpers scattered around `tesela-cli`,
//!    `tesela-server`, `tesela-backup` tests with `tiny()`. Single
//!    source of truth for what "a small valid mosaic" looks like.
//!
//! Same seed + same builder configuration ⇒ byte-identical output. Use
//! `MosaicBuilder::default().seed(...)` if you need that guarantee in
//! a test.

use anyhow::Result;
use chrono::{Duration, NaiveDate};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tesela_core::config::Config;

pub mod content;
pub mod rng;

use crate::content::{block_line, task_block, words, DEFAULT_TAGS, PAGE_NAMES};
use crate::rng::{chance, pick, range, rng, FixtureRng};

/// Builder for a synthetic mosaic. All fields have sensible defaults
/// — call `.build()` directly to get a small mosaic, or use one of the
/// `tiny()` / `medium()` / `large()` presets.
pub struct MosaicBuilder {
    seed: u64,
    daily_count: usize,
    page_count: usize,
    tasks: usize,
    backlinks_per_note: (usize, usize),
    attachments: Option<(usize, usize)>,
    tags: Vec<String>,
    deep_pages: usize,
}

impl Default for MosaicBuilder {
    fn default() -> Self {
        Self {
            seed: 42,
            daily_count: 30,
            page_count: 10,
            tasks: 20,
            backlinks_per_note: (0, 3),
            attachments: None,
            tags: DEFAULT_TAGS.iter().map(|s| s.to_string()).collect(),
            deep_pages: 0,
        }
    }
}

impl MosaicBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed(mut self, s: u64) -> Self {
        self.seed = s;
        self
    }

    pub fn daily_notes(mut self, n: usize) -> Self {
        self.daily_count = n;
        self
    }

    pub fn pages(mut self, n: usize) -> Self {
        self.page_count = n;
        self
    }

    pub fn tasks(mut self, n: usize) -> Self {
        self.tasks = n;
        self
    }

    pub fn backlinks_per_note(mut self, min: usize, max: usize) -> Self {
        self.backlinks_per_note = (min, max);
        self
    }

    pub fn attachments(mut self, count: usize, bytes_each: usize) -> Self {
        self.attachments = Some((count, bytes_each));
        self
    }

    pub fn tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Number of "deep" reference pages — long pages with many blocks
    /// and 5-7 levels of nesting. Stresses the parser + the typed-block
    /// query path.
    pub fn deep_pages(mut self, n: usize) -> Self {
        self.deep_pages = n;
        self
    }

    /// Materialize the mosaic into a fresh `TempDir`. The returned
    /// `MosaicHandle` owns the tempdir; drop it to clean up.
    pub fn build(self) -> Result<MosaicHandle> {
        let tmp = TempDir::new()?;
        let path = tmp.path().to_path_buf();
        let stats = self.build_at(&path)?;
        Ok(MosaicHandle {
            _tmp: Some(tmp),
            path,
            stats,
        })
    }

    /// Materialize at a caller-supplied path. Returns stats; caller
    /// owns cleanup.
    pub fn build_at(self, root: &Path) -> Result<MosaicStats> {
        fs::create_dir_all(root)?;
        let tesela_dir = root.join(".tesela");
        fs::create_dir_all(&tesela_dir)?;
        fs::create_dir_all(root.join("notes"))?;
        fs::create_dir_all(root.join("attachments"))?;

        Config::default().save(&tesela_dir.join("config.toml"))?;

        // Seed the default rail widgets so the synthetic mosaic feels
        // like a real one in `tesela tui` etc.
        let widgets_seeded = tesela_core::system_widgets::seed(root)? as i64;

        let mut rng = rng(self.seed);
        let notes_dir = root.join("notes");

        // Build the universe of note titles first so wikilinks can
        // resolve to any of them.
        let mut all_titles: Vec<String> = Vec::new();
        let today = chrono::Local::now().date_naive();
        for i in 0..self.daily_count {
            let date = today - Duration::days(i as i64);
            all_titles.push(date.format("%Y-%m-%d").to_string());
        }
        for i in 0..self.page_count {
            // Cycle through the canonical name pool with a numeric
            // suffix when we run out.
            let base = PAGE_NAMES[i % PAGE_NAMES.len()];
            let title = if i < PAGE_NAMES.len() {
                base.to_string()
            } else {
                format!("{}-{}", base, i / PAGE_NAMES.len() + 1)
            };
            all_titles.push(title);
        }

        let mut total_blocks: usize = 0;
        let mut total_tasks: usize = 0;
        let mut total_links: usize = 0;
        let mut tasks_left = self.tasks;

        // Daily notes
        for i in 0..self.daily_count {
            let date = today - Duration::days(i as i64);
            let id = date.format("%Y-%m-%d").to_string();
            let (body, blocks, tasks, links) = render_daily(
                &mut rng,
                &id,
                &all_titles,
                &self.tags,
                &mut tasks_left,
                self.backlinks_per_note,
                date,
            );
            let content = format!(
                "---\ntitle: \"{}\"\ntags: [\"daily\"]\ncreated: {}T00:00:00Z\n---\n{}",
                id, id, body,
            );
            fs::write(notes_dir.join(format!("{}.md", id)), content)?;
            total_blocks += blocks;
            total_tasks += tasks;
            total_links += links;
        }

        // Regular pages
        for i in 0..self.page_count {
            let title = all_titles[self.daily_count + i].clone();
            let is_deep = i < self.deep_pages;
            let (body, blocks, tasks, links) = render_page(
                &mut rng,
                &title,
                &all_titles,
                &self.tags,
                &mut tasks_left,
                self.backlinks_per_note,
                is_deep,
            );
            let note_type = if title.starts_with("person-") {
                Some("Person")
            } else if title.starts_with("project-") || title == "project-tracker" {
                Some("Project")
            } else {
                None
            };
            let mut content = String::new();
            content.push_str("---\n");
            content.push_str(&format!("title: \"{}\"\n", title));
            content.push_str("tags: []\n");
            if let Some(t) = note_type {
                content.push_str(&format!("type: \"{}\"\n", t));
            }
            content.push_str("---\n");
            content.push_str(&body);
            fs::write(notes_dir.join(format!("{}.md", title)), content)?;
            total_blocks += blocks;
            total_tasks += tasks;
            total_links += links;
        }

        // Attachments (binary blobs — we just need files with real bytes,
        // not actual valid images).
        let mut attachment_count: usize = 0;
        if let Some((n, bytes)) = self.attachments {
            for i in 0..n {
                let mut buf = vec![0u8; bytes];
                rand::RngCore::fill_bytes(&mut rng, &mut buf);
                fs::write(
                    root.join("attachments").join(format!("blob-{:04}.bin", i)),
                    buf,
                )?;
                attachment_count += 1;
            }
        }

        Ok(MosaicStats {
            notes: self.daily_count + self.page_count,
            daily_notes: self.daily_count,
            pages: self.page_count,
            blocks: total_blocks,
            tasks: total_tasks,
            links: total_links,
            attachments: attachment_count,
            widgets: widgets_seeded as usize,
        })
    }
}

fn render_daily(
    rng: &mut FixtureRng,
    id: &str,
    all_titles: &[String],
    tags: &[String],
    tasks_left: &mut usize,
    backlink_range: (usize, usize),
    _date: NaiveDate,
) -> (String, usize, usize, usize) {
    let block_count = range(rng, 6, 35);
    let mut lines: Vec<String> = Vec::with_capacity(block_count);
    let mut blocks = 0;
    let mut tasks = 0;
    let mut links = 0;

    // Filter out self-reference + cap the wikilink target pool to keep
    // a daily from looking like one giant wiki of links.
    let pool: Vec<String> = all_titles
        .iter()
        .filter(|t| t.as_str() != id)
        .cloned()
        .collect();

    for i in 0..block_count {
        let indent = if chance(rng, 0.25) { 1 } else { 0 };
        if *tasks_left > 0 && chance(rng, 0.20) {
            let deadline = if chance(rng, 0.5) {
                Some(format!(
                    "2026-{:02}-{:02}",
                    range(rng, 5, 12),
                    range(rng, 1, 28)
                ))
            } else {
                None
            };
            let task_lines = task_block(rng, indent, &pool, tags, deadline.as_deref());
            for line in &task_lines {
                if line.contains("[[") {
                    links += 1;
                }
            }
            blocks += task_lines.len();
            tasks += 1;
            *tasks_left -= 1;
            lines.extend(task_lines);
            continue;
        }
        let line = block_line(rng, indent, &pool, tags);
        if line.contains("[[") {
            links += 1;
        }
        // Maybe nest a child block.
        if i < block_count - 1 && indent == 0 && chance(rng, 0.30) {
            lines.push(line);
            blocks += 1;
            let child = block_line(rng, 1, &pool, tags);
            if child.contains("[[") {
                links += 1;
            }
            lines.push(child);
            blocks += 1;
        } else {
            lines.push(line);
            blocks += 1;
        }
    }

    // Inject extra backlinks per note (above and beyond what block_line
    // randomly added) — gives benches a stable backlink density to
    // measure.
    let extra = range(rng, backlink_range.0, backlink_range.1);
    for _ in 0..extra {
        if pool.is_empty() {
            break;
        }
        let line = format!("- see [[{}]]", pick(rng, &pool));
        lines.push(line);
        blocks += 1;
        links += 1;
    }

    let body = lines.join("\n") + "\n";
    (body, blocks, tasks, links)
}

fn render_page(
    rng: &mut FixtureRng,
    title: &str,
    all_titles: &[String],
    tags: &[String],
    tasks_left: &mut usize,
    backlink_range: (usize, usize),
    is_deep: bool,
) -> (String, usize, usize, usize) {
    let block_count = if is_deep {
        range(rng, 400, 1200)
    } else {
        range(rng, 5, 40)
    };
    let max_nest = if is_deep { 5 } else { 2 };
    let mut lines: Vec<String> = Vec::with_capacity(block_count);
    let mut blocks = 0;
    let mut tasks = 0;
    let mut links = 0;

    // Intro paragraph for non-deep pages.
    if !is_deep {
        let n = range(rng, 20, 60);
        lines.push(words(rng, n));
        lines.push(String::new());
    }

    let pool: Vec<String> = all_titles
        .iter()
        .filter(|t| t.as_str() != title)
        .cloned()
        .collect();

    let mut depth = 0;
    for i in 0..block_count {
        if *tasks_left > 0 && !is_deep && chance(rng, 0.10) {
            let deadline = if chance(rng, 0.4) {
                Some(format!(
                    "2026-{:02}-{:02}",
                    range(rng, 5, 12),
                    range(rng, 1, 28)
                ))
            } else {
                None
            };
            let task_lines = task_block(rng, depth, &pool, tags, deadline.as_deref());
            for line in &task_lines {
                if line.contains("[[") {
                    links += 1;
                }
            }
            blocks += task_lines.len();
            tasks += 1;
            *tasks_left -= 1;
            lines.extend(task_lines);
            continue;
        }

        let line = block_line(rng, depth, &pool, tags);
        if line.contains("[[") {
            links += 1;
        }
        lines.push(line);
        blocks += 1;

        // Depth walk — outline tends to nest a bit then pop back. Deep
        // pages drift further down than shallow ones.
        if depth < max_nest && chance(rng, 0.35) {
            depth += 1;
        } else if depth > 0 && chance(rng, 0.4) {
            depth -= 1;
        }
        let _ = i;
    }

    // Backlink injection — same as dailies.
    let extra = range(rng, backlink_range.0, backlink_range.1);
    for _ in 0..extra {
        if pool.is_empty() {
            break;
        }
        let line = format!("- relates to [[{}]]", pick(rng, &pool));
        lines.push(line);
        blocks += 1;
        links += 1;
    }

    let body = lines.join("\n") + "\n";
    (body, blocks, tasks, links)
}

/// A populated mosaic. Owns the underlying TempDir; drop to clean up.
pub struct MosaicHandle {
    _tmp: Option<TempDir>,
    pub path: PathBuf,
    pub stats: MosaicStats,
}

impl MosaicHandle {
    /// Detach from the tempdir owner so the mosaic survives the
    /// handle being dropped. The path is now the caller's
    /// responsibility to clean up.
    pub fn into_persistent(mut self) -> PathBuf {
        if let Some(tmp) = self._tmp.take() {
            // Persist by leaking the TempDir — `into_path` consumes
            // it and returns the path without deleting.
            let _ = tmp.keep();
        }
        self.path.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MosaicStats {
    pub notes: usize,
    pub daily_notes: usize,
    pub pages: usize,
    pub blocks: usize,
    pub tasks: usize,
    pub links: usize,
    pub attachments: usize,
    /// System widgets seeded under `notes/` (not counted in `notes`).
    pub widgets: usize,
}

// ──────────────────────────────────────────────────────────────────────
// Presets
// ──────────────────────────────────────────────────────────────────────

/// ~30 notes — drop-in replacement for the hand-rolled
/// `make_fixture_mosaic()` helpers in existing integration tests.
pub fn tiny() -> MosaicBuilder {
    MosaicBuilder::new()
        .daily_notes(20)
        .pages(8)
        .tasks(10)
        .backlinks_per_note(0, 2)
}

/// ~500 notes — matches the user's actual Logseq import (493 notes).
/// This is the headline target for regression benches.
pub fn medium() -> MosaicBuilder {
    MosaicBuilder::new()
        .daily_notes(420)
        .pages(80)
        .tasks(200)
        .backlinks_per_note(1, 5)
        .deep_pages(3)
}

/// ~2000 notes — headroom check for users with much larger graphs.
pub fn large() -> MosaicBuilder {
    MosaicBuilder::new()
        .daily_notes(1500)
        .pages(500)
        .tasks(800)
        .backlinks_per_note(2, 8)
        .deep_pages(10)
}
