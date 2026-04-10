import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";

import { parseBlocks, extractWikiLinks } from "@/lib/block-parser";
import type { ParsedBlock } from "@/lib/types/ParsedBlock";
import { BlockEditor } from "./BlockEditor";

/**
 * Renders a note's body as an indented block outliner.
 *
 * Each block shows as a bullet with display text. Tags render as pills,
 * wiki-links as clickable links. Clicking a block focuses it for editing
 * in a CM6 instance. On blur, the full body is reconstructed and saved.
 */
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
  const blocks = useMemo(() => parseBlocks(noteId, body), [noteId, body]);
  const [focusedBlockId, setFocusedBlockId] = useState<string | null>(null);
  const blocksRef = useRef(blocks);
  useEffect(() => {
    blocksRef.current = blocks;
  }, [blocks]);

  const handleBlockChange = useCallback(
    (blockId: string, newRawText: string) => {
      const updated = blocksRef.current.map((b) =>
        b.id === blockId ? { ...b, raw_text: newRawText } : b,
      );
      blocksRef.current = updated;

      // Reconstruct body from blocks
      const bodyLines = updated
        .map((b) => {
          const indent = "  ".repeat(b.indent_level);
          return `${indent}- ${b.raw_text}`;
        })
        .join("\n");

      onContentChange?.(`${frontmatter}${bodyLines}\n`);
    },
    [frontmatter, onContentChange],
  );

  if (blocks.length === 0) {
    return (
      <div className="text-sm text-muted-foreground italic">
        No blocks. Start typing with &quot;- &quot; to create one.
      </div>
    );
  }

  return (
    <div className="space-y-0.5">
      {blocks.map((block) => (
        <BlockItem
          key={block.id}
          block={block}
          isFocused={focusedBlockId === block.id}
          onFocus={() => setFocusedBlockId(block.id)}
          onBlur={() => setFocusedBlockId(null)}
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
  onChange,
}: {
  block: ParsedBlock;
  isFocused: boolean;
  onFocus: () => void;
  onBlur: () => void;
  onChange: (blockId: string, newRawText: string) => void;
}) {
  const indent = block.indent_level * 24;

  return (
    <div
      className="group flex items-start gap-1.5 rounded-sm hover:bg-accent/30 transition-colors"
      style={{ paddingLeft: `${indent}px` }}
    >
      <span className="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/50" />
      <div className="flex-1 min-w-0 py-0.5">
        {isFocused ? (
          <BlockEditor
            initialText={block.raw_text}
            onBlur={onBlur}
            onChange={(text) => onChange(block.id, text)}
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

/** Renders a block's display text with wiki-links as clickable links. */
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

/** Renders a property value, turning [[links]] into clickable links. */
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
