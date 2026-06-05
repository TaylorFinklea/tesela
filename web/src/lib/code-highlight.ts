/**
 * Minimal, dependency-free syntax highlighter for fenced code blocks.
 *
 * A real grammar-based highlighter (highlight.js / Shiki) is overkill for a
 * notes app and either bloats the bundle or needs async init that fights the
 * synchronous CodeMirror decoration build. This is a small left-to-right
 * scanner that emits the high-value token kinds — comments, strings, numbers,
 * booleans, and per-language keywords — as `hljs-*`-classed spans, so it
 * reuses any highlight.js-style theme CSS.
 *
 * Returns HTML-escaped markup safe to assign to innerHTML (code text is
 * escaped; only our own span tags are emitted).
 */

type LangSpec = {
  /** Line-comment lead tokens (longest first). */
  line: string[];
  /** Block comment [open, close], if any. */
  block?: [string, string];
  /** String delimiters. */
  strings: string[];
  keywords: Set<string>;
  literals: Set<string>;
};

const C_LIKE_KW =
  "as async await break case catch class const continue default delete do else enum export extends false finally for from function if implements import in instanceof interface let new null of return static super switch this throw true try type typeof var void while yield with public private protected readonly";
const PY_KW =
  "and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield True False None match case";
const SH_KW =
  "if then else elif fi for while do done case esac in function select until return local export readonly declare echo cd export source alias set unset";
const RUST_KW =
  "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while";
const GO_KW =
  "break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var nil true false";
const SQL_KW =
  "select from where insert into values update set delete create table drop alter add primary key foreign references join left right inner outer on group by order having limit offset distinct as and or not null is in between like union all index view default";

const set = (s: string) => new Set(s.split(/\s+/).filter(Boolean));

const SPECS: Record<string, LangSpec> = {
  js: { line: ["//"], block: ["/*", "*/"], strings: ['"', "'", "`"], keywords: set(C_LIKE_KW), literals: set("true false null undefined NaN") },
  bash: { line: ["#"], strings: ['"', "'"], keywords: set(SH_KW), literals: set("true false") },
  python: { line: ["#"], strings: ['"', "'"], keywords: set(PY_KW), literals: set("True False None") },
  rust: { line: ["//"], block: ["/*", "*/"], strings: ['"'], keywords: set(RUST_KW), literals: set("true false") },
  go: { line: ["//"], block: ["/*", "*/"], strings: ['"', "`"], keywords: set(GO_KW), literals: set("true false nil") },
  sql: { line: ["--"], block: ["/*", "*/"], strings: ["'"], keywords: set(SQL_KW.toLowerCase()), literals: set("true false null") },
  json: { line: [], strings: ['"'], keywords: set(""), literals: set("true false null") },
  yaml: { line: ["#"], strings: ['"', "'"], keywords: set(""), literals: set("true false null yes no") },
  css: { line: [], block: ["/*", "*/"], strings: ['"', "'"], keywords: set(""), literals: set("") },
};

const ALIAS: Record<string, string> = {
  sh: "bash", shell: "bash", zsh: "bash", console: "bash",
  ts: "js", typescript: "js", javascript: "js", jsx: "js", tsx: "js", mjs: "js",
  py: "python", rs: "rust", golang: "go", postgres: "sql", postgresql: "sql", mysql: "sql",
  yml: "yaml", scss: "css", less: "css",
};

function specFor(lang: string): LangSpec {
  const key = ALIAS[lang] ?? lang;
  return SPECS[key] ?? SPECS.js; // generic-ish C-like fallback
}

const IDENT_START = /[A-Za-z_$]/;
const IDENT_PART = /[A-Za-z0-9_$]/;

export type CodeToken = { start: number; end: number; kind: "comment" | "string" | "number" | "keyword" | "literal" };

/** Tokenize `code` for `lang`, returning highlighted token spans (offsets into
 *  `code`). Plain text yields no token. Offsets map straight onto a CodeMirror
 *  doc when the caller adds `contentStart` (handles multi-line constructs). */
export function tokenizeCode(code: string, lang: string): CodeToken[] {
  const spec = specFor((lang || "").trim().toLowerCase());
  const out: CodeToken[] = [];
  let i = 0;
  const n = code.length;
  const startsWith = (tok: string) => code.startsWith(tok, i);

  while (i < n) {
    const ch = code[i];

    if (spec.block && startsWith(spec.block[0])) {
      const end = code.indexOf(spec.block[1], i + spec.block[0].length);
      const stop = end === -1 ? n : end + spec.block[1].length;
      out.push({ start: i, end: stop, kind: "comment" });
      i = stop;
      continue;
    }
    if (spec.line.some((t) => startsWith(t))) {
      let end = code.indexOf("\n", i);
      if (end === -1) end = n;
      out.push({ start: i, end, kind: "comment" });
      i = end;
      continue;
    }
    if (spec.strings.includes(ch)) {
      let j = i + 1;
      while (j < n && code[j] !== ch) {
        if (code[j] === "\\") j++;
        else if (code[j] === "\n") break; // single-line strings only (avoid runaway)
        j++;
      }
      j = Math.min(j + 1, n);
      out.push({ start: i, end: j, kind: "string" });
      i = j;
      continue;
    }
    if (/[0-9]/.test(ch) || (ch === "." && /[0-9]/.test(code[i + 1] ?? ""))) {
      let j = i + 1;
      while (j < n && /[0-9a-fA-FxX._]/.test(code[j])) j++;
      out.push({ start: i, end: j, kind: "number" });
      i = j;
      continue;
    }
    if (IDENT_START.test(ch)) {
      let j = i + 1;
      while (j < n && IDENT_PART.test(code[j])) j++;
      const word = code.slice(i, j);
      if (spec.literals.has(word)) out.push({ start: i, end: j, kind: "literal" });
      else if (spec.keywords.has(word)) out.push({ start: i, end: j, kind: "keyword" });
      i = j;
      continue;
    }
    i++;
  }
  return out;
}
