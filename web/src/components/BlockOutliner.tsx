import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";

import { parseBlocks, extractWikiLinks } from "@/lib/block-parser";
import type { ParsedBlock } from "@/lib/types/ParsedBlock";
import { BlockEditor } from "./BlockEditor";

export function BlockOutliner({
  noteId,
  body,
  frontmatter,
  onContentChange,
}: {
  noteId: string;
  body: string;
  frontmatter: string;
  onContentChange?: (fullContent: string) => void;
}) {
  const initialBlocks = useMemo(() => parseBlocks(noteId, body), [noteId, body]);
  const [blocks, setBlocks] = useState(initialBlocks);
  const [focusedIndex, setFocusedIndex] = useState<number | null>(null);
  const blocksRef = useRef(blocks);

  useEffect(() => {
    blocksRef.current = blocks;
  }, [blocks]);

  const saveBlocks = useCallback(
    (updated: ParsedBlock[]) => {
      const bodyLines = updated
        .map((b) => {
          const indent = "  ".repeat(b.indent_level);
          const contIndent = indent + "  "; // continuation lines get one extra level
          const lines = b.raw_text.split("\n");
          const first = `${indent}- ${lines[0]}`;
          const rest = lines.slice(1).map((l) => `${contIndent}${l}`);
          return [first, ...rest].join("\n");
        })
        .join("\n");
      onContentChange?.(`${frontmatter}${bodyLines}\n`);
    },
    [frontmatter, onContentChange],
  );

  const handleBlockChange = useCallback(
    (blockId: string, newRawText: string) => {
      setBlocks((prev) => {
        const updated = prev.map((b) =>
          b.id === blockId ? { ...b, raw_text: newRawText, text: newRawText.split("\n")[0]?.replace(/#([A-Za-z0-9_/-]+)/g, "").trim() ?? "" } : b,
        );
        blocksRef.current = updated;
        saveBlocks(updated);
        return updated;
      });
    },
    [saveBlocks],
  );

  const handleNavigate = useCallback(
    (direction: "up" | "down") => {
      setFocusedIndex((current) => {
        if (current === null) return null;
        const next =
          direction === "up"
            ? Math.max(0, current - 1)
            : Math.min(blocksRef.current.length - 1, current + 1);
        return next;
      });
    },
    [],
  );

  const handleEscape = useCallback(() => {
    setFocusedIndex(null);
  }, []);

  const handleEnter = useCallback(
    (atIndex: number) => {
      setBlocks((prev) => {
        const current = prev[atIndex];
        if (!current) return prev;
        const newBlock: ParsedBlock = {
          id: `${noteId}:new-${Date.now()}`,
          text: "",
          raw_text: "",
          tags: [],
          properties: {},
          indent_level: current.indent_level,
          note_id: noteId,
        };
        const updated = [...prev.slice(0, atIndex + 1), newBlock, ...prev.slice(atIndex + 1)];
        blocksRef.current = updated;
        saveBlocks(updated);
        return updated;
      });
      setFocusedIndex(atIndex + 1);
    },
    [noteId, saveBlocks],
  );

  const handleIndent = useCallback(
    (atIndex: number, direction: "indent" | "outdent") => {
      setBlocks((prev) => {
        const block = prev[atIndex];
        if (!block) return prev;
        const newLevel =
          direction === "indent"
            ? block.indent_level + 1
            : Math.max(0, block.indent_level - 1);
        if (newLevel === block.indent_level) return prev;
        const updated = prev.map((b, i) =>
          i === atIndex ? { ...b, indent_level: newLevel } : b,
        );
        blocksRef.current = updated;
        saveBlocks(updated);
        return updated;
      });
    },
    [saveBlocks],
  );

  const handleBackspace = useCallback(
    (atIndex: number) => {
      setBlocks((prev) => {
        const block = prev[atIndex];
        if (!block || block.raw_text !== "") return prev; // Only delete empty blocks
        if (prev.length <= 1) return prev; // Don't delete the last block
        const updated = prev.filter((_, i) => i !== atIndex);
        blocksRef.current = updated;
        saveBlocks(updated);
        return updated;
      });
      setFocusedIndex((current) => {
        if (current === null || current === 0) return current;
        return current - 1;
      });
    },
    [saveBlocks],
  );

  // Keyboard nav when no block is focused
  const containerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = containerRef.current;
    if (!el || focusedIndex !== null) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown" || e.key === "j") {
        e.preventDefault();
        setFocusedIndex(0);
      }
    };
    el.addEventListener("keydown", handler);
    return () => el.removeEventListener("keydown", handler);
  }, [focusedIndex]);

  if (blocks.length === 0) {
    return (
      <div className="text-sm text-muted-foreground italic">
        No blocks. Start typing with &quot;- &quot; to create one.
      </div>
    );
  }

  return (
    <div ref={containerRef} className="space-y-0.5" tabIndex={-1}>
      {blocks.map((block, index) => (
        <BlockItem
          key={block.id}
          block={block}
          isFocused={focusedIndex === index}
          onFocus={() => setFocusedIndex(index)}
          onBlur={() => {
            setFocusedIndex((current) =>
              current === index ? null : current,
            );
          }}
          onNavigate={handleNavigate}
          onEscape={handleEscape}
          onEnter={() => handleEnter(index)}
          onIndent={(dir) => handleIndent(index, dir)}
          onBackspaceEmpty={() => handleBackspace(index)}
          onChange={handleBlockChange}
        />
      ))}
    </div>
  );
}

function BlockItem({
  block,
  isFocused,
  onFocus,
  onBlur,
  onNavigate,
  onEscape,
  onEnter,
  onIndent,
  onBackspaceEmpty,
  onChange,
}: {
  block: ParsedBlock;
  isFocused: boolean;
  onFocus: () => void;
  onBlur: () => void;
  onNavigate: (direction: "up" | "down") => void;
  onEscape: () => void;
  onEnter: () => void;
  onIndent: (direction: "indent" | "outdent") => void;
  onBackspaceEmpty: () => void;
  onChange: (blockId: string, newRawText: string) => void;
}) {
  const indent = block.indent_level * 24;

  return (
    <div
      className={`group flex items-start gap-1.5 rounded-sm transition-colors ${
        isFocused ? "bg-accent/20" : "hover:bg-accent/30"
      }`}
      style={{ paddingLeft: `${indent}px` }}
    >
      <span className="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/50" />
      <div className="flex-1 min-w-0 py-0.5">
        {isFocused ? (
          <BlockEditor
            initialText={block.raw_text}
            onBlur={onBlur}
            onChange={(text: string) => onChange(block.id, text)}
            onNavigate={onNavigate}
            onEscape={onEscape}
            onEnter={onEnter}
            onIndent={onIndent}
            onBackspaceEmpty={onBackspaceEmpty}
          />
        ) : (
          <div
            className="text-sm leading-relaxed cursor-text min-h-[24px]"
            onClick={onFocus}
          >
            <BlockDisplayText block={block} />
            {block.tags.length > 0 && (
              <span className="ml-2 inline-flex gap-1">
                {block.tags.map((tag) => (
                  <Link
                    key={tag}
                    href={`/p/${encodeURIComponent(tag.toLowerCase())}`}
                    className="text-xs px-1.5 py-0.5 rounded bg-accent text-accent-foreground hover:bg-accent/80"
                    onClick={(e) => e.stopPropagation()}
                  >
                    #{tag}
                  </Link>
                ))}
              </span>
            )}
            {Object.keys(block.properties).length > 0 && (
              <div className="mt-0.5 space-y-0">
                {Object.entries(block.properties).map(([key, value]) => (
                  <div key={key} className="text-xs text-muted-foreground">
                    <span className="text-muted-foreground/70">{key}::</span>{" "}
                    <PropertyValue value={value} />
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function BlockDisplayText({ block }: { block: ParsedBlock }) {
  const text = block.text;
  const links = extractWikiLinks(text);

  if (links.length === 0) {
    return <span>{text}</span>;
  }

  const parts: React.ReactNode[] = [];
  let lastEnd = 0;
  for (const link of links) {
    if (link.start > lastEnd) {
      parts.push(<span key={`t${lastEnd}`}>{text.slice(lastEnd, link.start)}</span>);
    }
    parts.push(
      <Link
        key={`l${link.start}`}
        href={`/p/${encodeURIComponent(link.target.toLowerCase())}`}
        className="text-primary underline underline-offset-2 decoration-primary/40 hover:decoration-primary"
        onClick={(e) => e.stopPropagation()}
      >
        {link.display}
      </Link>,
    );
    lastEnd = link.end;
  }
  if (lastEnd < text.length) {
    parts.push(<span key={`t${lastEnd}`}>{text.slice(lastEnd)}</span>);
  }

  return <>{parts}</>;
}

function PropertyValue({ value }: { value: string }) {
  const links = extractWikiLinks(value);
  if (links.length === 0) return <span>{value}</span>;

  const parts: React.ReactNode[] = [];
  let lastEnd = 0;
  for (const link of links) {
    if (link.start > lastEnd) {
      parts.push(<span key={`t${lastEnd}`}>{value.slice(lastEnd, link.start)}</span>);
    }
    parts.push(
      <Link
        key={`l${link.start}`}
        href={`/p/${encodeURIComponent(link.target.toLowerCase())}`}
        className="text-primary underline underline-offset-2 decoration-primary/40 hover:decoration-primary"
      >
        {link.display}
      </Link>,
    );
    lastEnd = link.end;
  }
  if (lastEnd < value.length) {
    parts.push(<span key={`t${lastEnd}`}>{value.slice(lastEnd)}</span>);
  }
  return <>{parts}</>;
}
