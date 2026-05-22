<script lang="ts">
  import { onMount } from "svelte";
  import { parseDateAndRecurrenceInput, parseRecurrenceInput } from "$lib/date-parser";

  let {
    initialDate,
    initialTime,
    initialRecurrence,
    position,
    onPick,
    onClose,
  }: {
    /** ISO date string `YYYY-MM-DD`. Defaults to today. */
    initialDate?: string;
    /** Optional 24-hour `HH:mm`. */
    initialTime?: string | null;
    /** Canonical recurrence string (e.g. `"monthly"`, `"every 2 weeks"`)
     *  or `null` for non-recurring. */
    initialRecurrence?: string | null;
    position: { x: number; y: number };
    /** Phase 12.2 — recurrence is the third tuple element. `null` means
     *  "non-recurring" (and clears any existing `recurring::`). */
    onPick: (iso: string, time: string | null, recurrence: string | null) => void;
    onClose: () => void;
  } = $props();

  function parseISO(s: string | undefined): Date {
    if (s) {
      const [y, m, d] = s.split("-").map(Number);
      if (!Number.isNaN(y + m + d)) return new Date(y, m - 1, d);
    }
    return new Date();
  }
  function fmt(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  }

  let selected = $state<Date>(parseISO(initialDate));
  let viewMonth = $state<Date>(new Date(selected.getFullYear(), selected.getMonth(), 1));
  let nlInput = $state<string>("");
  // Optional 24-hour `HH:mm` part of the value. The user can either type
  // it as part of the NL input ("fri at 10am", "tom 14:30") or edit the
  // time field below the calendar directly. `null` means no time set
  // (date-only) — that's the default.
  let selectedTime = $state<string | null>(initialTime ?? null);
  // Phase 12.2 — canonical recurrence string (or null for non-recurring).
  // Buttons set this to one of the presets; the custom input round-trips
  // through `parseRecurrenceInput` so only valid values land here.
  let selectedRecurrence = $state<string | null>(null);
  // Custom recurrence input — only visible when the "custom" chord is on
  // or the current selectedRecurrence isn't one of the preset chips.
  const PRESETS = ["daily", "weekly", "monthly", "yearly", "weekdays", "weekends"] as const;
  let customRecurrenceOpen = $state<boolean>(false);
  let customRecurrenceInput = $state<string>("");

  // Day-of-week toggle row — for BYDAY recurrences like "every mon, wed, fri".
  const WEEKDAYS_TOGGLE = [
    { key: "mon", label: "M" },
    { key: "tue", label: "T" },
    { key: "wed", label: "W" },
    { key: "thu", label: "T" },
    { key: "fri", label: "F" },
    { key: "sat", label: "S" },
    { key: "sun", label: "S" },
  ];
  let pickedDays = $state<Set<string>>(new Set());

  // End-condition control.
  let endMode = $state<"never" | "until" | "count">("never");
  let endUntil = $state<string>("");  // YYYY-MM-DD
  let endCount = $state<number>(1);

  const endClause = $derived<string>(
    endMode === "until" && endUntil
      ? ` until ${endUntil}`
      : endMode === "count" && endCount >= 1
        ? ` count ${endCount}`
        : "",
  );

  // The value committed via onPick: selectedRecurrence + endClause, or null.
  const committedRecurrence = $derived<string | null>(
    selectedRecurrence ? selectedRecurrence + endClause : null,
  );

  // Parse `initialRecurrence` to seed all controls on open.
  function initFromRecurrence(raw: string | null | undefined) {
    if (!raw) {
      selectedRecurrence = null;
      customRecurrenceOpen = false;
      customRecurrenceInput = "";
      pickedDays = new Set();
      endMode = "never";
      endUntil = "";
      endCount = 1;
      return;
    }

    // Split off end clause first.
    let base = raw;
    const untilIdx = raw.lastIndexOf(" until ");
    const countIdx = raw.lastIndexOf(" count ");
    if (untilIdx !== -1) {
      endUntil = raw.slice(untilIdx + 7).trim();
      endMode = "until";
      base = raw.slice(0, untilIdx);
    } else if (countIdx !== -1) {
      const n = Number(raw.slice(countIdx + 7).trim());
      endCount = Number.isFinite(n) && n >= 1 ? n : 1;
      endMode = "count";
      base = raw.slice(0, countIdx);
    } else {
      endMode = "never";
      endUntil = "";
      endCount = 1;
    }

    // Check if the base is a BYDAY string.
    if (/^every (mon|tue|wed|thu|fri|sat|sun)(,|$)/.test(base)) {
      const tokens = base.slice(6).split(",").map((t) => t.trim());
      pickedDays = new Set(tokens);
      selectedRecurrence = base;
      customRecurrenceOpen = false;
      customRecurrenceInput = "";
      return;
    }

    // Check if the base is a simple preset.
    if ((PRESETS as readonly string[]).includes(base)) {
      selectedRecurrence = base;
      pickedDays = new Set();
      customRecurrenceOpen = false;
      customRecurrenceInput = "";
      return;
    }

    // Anything else goes into the custom input.
    selectedRecurrence = base;
    pickedDays = new Set();
    customRecurrenceOpen = true;
    customRecurrenceInput = raw; // full string (with end clause) in custom box
  }

  /** Live parser result. `null` when input is empty or unparseable. */
  const parsedFromInput = $derived.by(() => {
    if (!nlInput.trim()) return null;
    return parseDateAndRecurrenceInput(nlInput);
  });

  // When the user types a recognizable phrase, jump the highlighted day +
  // the visible month to match. Don't fight the user's calendar clicks —
  // only react when the input actually changes.
  let lastNlInput = "";
  $effect(() => {
    if (nlInput === lastNlInput) return;
    lastNlInput = nlInput;
    if (parsedFromInput) {
      const d = parseISO(parsedFromInput.date);
      selected = d;
      viewMonth = new Date(d.getFullYear(), d.getMonth(), 1);
      // Only overwrite time when the user actually typed one; preserve
      // the existing selection when their NL is date-only.
      if (parsedFromInput.time !== null) selectedTime = parsedFromInput.time;
      // Same shape for recurrence — only adopt when the user typed a tail
      // ("fri weekly" → recurrence=weekly).
      if (parsedFromInput.recurrence !== null) {
        if ((PRESETS as readonly string[]).includes(parsedFromInput.recurrence)) {
          selectedRecurrence = parsedFromInput.recurrence;
          pickedDays = new Set();
          customRecurrenceOpen = false;
        } else {
          customRecurrenceOpen = true;
          customRecurrenceInput = parsedFromInput.recurrence;
          selectedRecurrence = parsedFromInput.recurrence;
          pickedDays = new Set();
        }
        // NL input carries its own end clause — don't compose with UI end controls.
        endMode = "never";
      }
    }
  });

  const today = $derived(fmt(new Date()));

  const grid = $derived.by(() => {
    const first = new Date(viewMonth.getFullYear(), viewMonth.getMonth(), 1);
    const lastDay = new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 0).getDate();
    const offset = (first.getDay() + 6) % 7;
    const days: { date: Date; inMonth: boolean }[] = [];
    for (let i = offset; i > 0; i--) {
      const d = new Date(first);
      d.setDate(d.getDate() - i);
      days.push({ date: d, inMonth: false });
    }
    for (let d = 1; d <= lastDay; d++) {
      days.push({ date: new Date(viewMonth.getFullYear(), viewMonth.getMonth(), d), inMonth: true });
    }
    while (days.length % 7 !== 0) {
      const last = days[days.length - 1].date;
      const d = new Date(last);
      d.setDate(d.getDate() + 1);
      days.push({ date: d, inMonth: false });
    }
    const rows: { date: Date; inMonth: boolean }[][] = [];
    for (let i = 0; i < days.length; i += 7) rows.push(days.slice(i, i + 7));
    return rows;
  });

  const monthLabel = $derived(
    viewMonth.toLocaleDateString(undefined, { month: "long", year: "numeric" }),
  );

  function move(days: number) {
    const next = new Date(selected);
    next.setDate(next.getDate() + days);
    selected = next;
    viewMonth = new Date(next.getFullYear(), next.getMonth(), 1);
  }
  function prevMonth() {
    viewMonth = new Date(viewMonth.getFullYear(), viewMonth.getMonth() - 1, 1);
  }
  function nextMonth() {
    viewMonth = new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 1);
  }

  // Two focus modes: input (typing natural language) vs grid (vim-style
  // calendar nav). Tab toggles. Letters like h/j/k/l would conflict with
  // typing into the NL input, so they only act as nav in grid mode.
  let focusMode = $state<"input" | "grid">("input");

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      onPick(fmt(selected), selectedTime, committedRecurrence);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      if (focusMode === "grid") {
        focusMode = "input";
        requestAnimationFrame(() => inputEl?.focus());
        return;
      }
      onClose();
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      if (e.shiftKey) {
        focusMode = "input";
        requestAnimationFrame(() => inputEl?.focus());
      } else {
        focusMode = "grid";
        // Focus the container directly — the browser blurs the input with
        // relatedTarget=containerEl, which handleBlur recognizes as
        // in-dialog and doesn't trigger onClose. Calling inputEl.blur()
        // first would set relatedTarget=null and close the dialog.
        containerEl?.focus();
      }
      return;
    }
    // Arrow keys nav the calendar even when input is focused. The input is
    // single-line and rarely needs caret-arrow editing; calendar nav is the
    // higher-value behavior here.
    if (e.key === "ArrowLeft") { e.preventDefault(); move(e.shiftKey ? -7 : -1); return; }
    if (e.key === "ArrowRight") { e.preventDefault(); move(e.shiftKey ? 7 : 1); return; }
    if (e.key === "ArrowUp") { e.preventDefault(); move(-7); return; }
    if (e.key === "ArrowDown") { e.preventDefault(); move(7); return; }
    // hjkl only in grid mode (else they'd type into the NL input).
    if (focusMode === "grid") {
      if (e.key === "h") { e.preventDefault(); move(e.shiftKey ? -7 : -1); return; }
      if (e.key === "l") { e.preventDefault(); move(e.shiftKey ? 7 : 1); return; }
      if (e.key === "k") { e.preventDefault(); move(-7); return; }
      if (e.key === "j") { e.preventDefault(); move(7); return; }
    }
  }

  let inputEl = $state<HTMLInputElement | null>(null);
  let containerEl = $state<HTMLDivElement | null>(null);

  onMount(() => {
    initFromRecurrence(initialRecurrence);
    inputEl?.focus();
  });

  // If focus drifts off the dialog (e.g. user clicked a day cell), close.
  function handleBlur(e: FocusEvent) {
    const next = e.relatedTarget as Node | null;
    if (next && containerEl?.contains(next)) return;
    onClose();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={containerEl}
  role="dialog"
  aria-label="Date picker"
  tabindex="-1"
  class="fixed z-50 bg-popover border border-border rounded-md shadow-xl p-2 outline-none"
  style="left: {position.x}px; top: {position.y}px;"
  onkeydown={handleKey}
>
  <!-- NL input -->
  <input
    bind:this={inputEl}
    bind:value={nlInput}
    onblur={handleBlur}
    placeholder="Type: today, fri, in 3 days…"
    class="w-full text-[12px] px-2 py-1 mb-2 rounded bg-muted/30 border border-border/40
           text-foreground placeholder:text-muted-foreground/40
           focus:outline-none focus:border-primary/40
           {nlInput.trim() && !parsedFromInput ? 'border-destructive/40' : ''}"
  />

  <!-- Header: month nav -->
  <div class="flex items-center justify-between mb-2 px-1">
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[12px] px-1.5 text-muted-foreground/60 hover:text-foreground/80 rounded hover:bg-muted/40"
      onclick={prevMonth}
      onblur={handleBlur}
      title="Previous month"
    >‹</button>
    <span class="text-[12px] font-medium text-foreground/90">{monthLabel}</span>
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[12px] px-1.5 text-muted-foreground/60 hover:text-foreground/80 rounded hover:bg-muted/40"
      onclick={nextMonth}
      onblur={handleBlur}
      title="Next month"
    >›</button>
  </div>

  <!-- Day-of-week header -->
  <div class="grid grid-cols-7 text-[10px] text-muted-foreground/50 uppercase tracking-wider mb-1">
    <span class="text-center">Mo</span>
    <span class="text-center">Tu</span>
    <span class="text-center">We</span>
    <span class="text-center">Th</span>
    <span class="text-center">Fr</span>
    <span class="text-center">Sa</span>
    <span class="text-center">Su</span>
  </div>

  <!-- Day grid -->
  <div class="grid grid-cols-7 gap-px text-[12px]">
    {#each grid as row}
      {#each row as cell}
        {@const iso = fmt(cell.date)}
        {@const isSelected = iso === fmt(selected)}
        {@const isToday = iso === today}
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          class="
            w-7 h-7 rounded text-center transition-colors
            {isSelected ? 'bg-primary text-primary-foreground' : ''}
            {!isSelected && isToday ? 'ring-1 ring-primary/40' : ''}
            {!isSelected && cell.inMonth ? 'text-foreground/85 hover:bg-muted/40' : ''}
            {!cell.inMonth ? 'text-muted-foreground/30 hover:bg-muted/30' : ''}
          "
          onclick={() => { selected = cell.date; viewMonth = new Date(cell.date.getFullYear(), cell.date.getMonth(), 1); onPick(iso, selectedTime, committedRecurrence); }}
          onblur={handleBlur}
          title={iso}
        >{cell.date.getDate()}</button>
      {/each}
    {/each}
  </div>

  <!-- Time field — optional 24-hour HH:mm. Empty input means date-only.
       The NL input above also recognizes "fri at 10am" / "tom 14:30" and
       fills this in; this field is the explicit edit/clear escape hatch. -->
  <div class="flex items-center gap-2 mt-2 px-1">
    <span class="text-[10px] text-muted-foreground/50 uppercase tracking-wider">time</span>
    <input
      type="time"
      value={selectedTime ?? ""}
      onchange={(e) => {
        const v = (e.target as HTMLInputElement).value;
        selectedTime = v ? v : null;
      }}
      class="flex-1 text-[12px] px-2 py-0.5 rounded bg-muted/30 border border-border/40
             text-foreground focus:outline-none focus:border-primary/40"
    />
    {#if selectedTime}
      <!-- svelte-ignore a11y_consider_explicit_label -->
      <button
        class="text-[10px] text-muted-foreground/50 hover:text-foreground/70 px-1.5 py-0.5"
        onclick={() => (selectedTime = null)}
        title="Clear time"
      >×</button>
    {/if}
  </div>

  <!-- Recurrence sub-row — Phase 12.2. Picks the canonical string we'll
       store on `recurring::`. `none` clears it; `custom` reveals a text
       field that runs through `parseRecurrenceInput` (e.g. "every 3 weeks"). -->
  <div class="flex items-center gap-1 mt-2 px-1 flex-wrap">
    <span class="text-[10px] text-muted-foreground/50 uppercase tracking-wider mr-1">repeat</span>
    {#each [
      { label: "none", value: null },
      { label: "daily", value: "daily" },
      { label: "weekly", value: "weekly" },
      { label: "monthly", value: "monthly" },
      { label: "yearly", value: "yearly" },
      { label: "weekdays", value: "weekdays" },
      { label: "weekends", value: "weekends" },
    ] as opt}
      {@const active = selectedRecurrence === opt.value && pickedDays.size === 0 && !customRecurrenceOpen}
      <!-- svelte-ignore a11y_consider_explicit_label -->
      <button
        class="text-[10px] px-1.5 py-0.5 rounded border transition-colors
               {active ? 'bg-primary text-primary-foreground border-primary' : 'border-border/40 text-foreground/70 hover:bg-muted/40'}"
        onclick={() => { selectedRecurrence = opt.value; pickedDays = new Set(); customRecurrenceOpen = false; }}
        onblur={handleBlur}
      >{opt.label}</button>
    {/each}
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[10px] px-1.5 py-0.5 rounded border transition-colors
             {customRecurrenceOpen ? 'bg-primary text-primary-foreground border-primary' : 'border-border/40 text-foreground/70 hover:bg-muted/40'}"
      onclick={() => {
        customRecurrenceOpen = !customRecurrenceOpen;
        if (customRecurrenceOpen) {
          pickedDays = new Set();
          endMode = "never";
          if (customRecurrenceInput) {
            const rec = parseRecurrenceInput(customRecurrenceInput);
            if (rec) selectedRecurrence = rec;
          }
        }
      }}
      onblur={handleBlur}
    >custom</button>
  </div>

  <!-- Day-of-week toggle row — builds BYDAY recurrences like "every mon, wed, fri". -->
  <div class="flex items-center gap-1 mt-1.5 px-1">
    <span class="text-[10px] text-muted-foreground/50 uppercase tracking-wider mr-1">days</span>
    {#each WEEKDAYS_TOGGLE as d}
      <!-- svelte-ignore a11y_consider_explicit_label -->
      <button
        class="text-[10px] w-5 h-5 rounded border transition-colors
               {pickedDays.has(d.key) ? 'bg-primary text-primary-foreground border-primary' : 'border-border/40 text-foreground/70 hover:bg-muted/40'}"
        onclick={() => {
          const next = new Set(pickedDays);
          if (next.has(d.key)) {
            next.delete(d.key);
          } else {
            next.add(d.key);
          }
          pickedDays = next;
          if (pickedDays.size === 0) {
            selectedRecurrence = null;
          } else {
            // Sort picked days in Mon-first order.
            const ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
            const sorted = [...pickedDays].sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
            selectedRecurrence = `every ${sorted.join(", ")}`;
            customRecurrenceOpen = false;
          }
        }}
        onblur={handleBlur}
        title={d.key}
      >{d.label}</button>
    {/each}
  </div>

  <!-- End-condition control: Never / Until / After. -->
  <div class="flex items-center gap-1 mt-1.5 px-1 flex-wrap">
    <span class="text-[10px] text-muted-foreground/50 uppercase tracking-wider mr-1">end</span>
    {#each (["never", "until", "count"] as const) as mode}
      {@const label = mode === "never" ? "never" : mode === "until" ? "until" : "after"}
      <!-- svelte-ignore a11y_consider_explicit_label -->
      <button
        class="text-[10px] px-1.5 py-0.5 rounded border transition-colors
               {endMode === mode ? 'bg-primary text-primary-foreground border-primary' : 'border-border/40 text-foreground/70 hover:bg-muted/40'}"
        onclick={() => { endMode = mode; }}
        onblur={handleBlur}
      >{label}</button>
    {/each}
    {#if endMode === "until"}
      <input
        type="date"
        bind:value={endUntil}
        onblur={handleBlur}
        class="text-[11px] px-1.5 py-0.5 rounded bg-muted/30 border border-border/40
               text-foreground focus:outline-none focus:border-primary/40 ml-1"
      />
    {:else if endMode === "count"}
      <input
        type="number"
        min="1"
        bind:value={endCount}
        onblur={handleBlur}
        class="text-[11px] w-12 px-1.5 py-0.5 rounded bg-muted/30 border border-border/40
               text-foreground focus:outline-none focus:border-primary/40 ml-1"
      />
      <span class="text-[10px] text-muted-foreground/60">times</span>
    {/if}
  </div>

  {#if customRecurrenceOpen}
    {@const customParse = parseRecurrenceInput(customRecurrenceInput)}
    <input
      bind:value={customRecurrenceInput}
      onblur={handleBlur}
      placeholder='e.g. "every 3 weeks"'
      class="w-full text-[12px] px-2 py-1 mt-1 rounded bg-muted/30 border
             text-foreground placeholder:text-muted-foreground/40
             focus:outline-none focus:border-primary/40
             {customRecurrenceInput.trim() && !customParse ? 'border-destructive/40' : 'border-border/40'}"
      oninput={() => {
        const rec = parseRecurrenceInput(customRecurrenceInput);
        selectedRecurrence = rec;
        // Custom text carries its own end clause — reset UI end controls.
        pickedDays = new Set();
        endMode = "never";
      }}
    />
  {/if}

  <!-- Footer hint — adapts to focus mode so the user always sees the keys
       that are live right now. -->
  <div class="text-[10px] text-muted-foreground/40 mt-1.5 px-1 text-center">
    {#if focusMode === "grid"}
      hjkl/arrows · enter · esc · S-tab → input
    {:else}
      type · arrows · tab → grid · enter · esc
    {/if}
  </div>
</div>
