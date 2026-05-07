<script lang="ts">
  // Sandbox: four interactive prototypes for redesigning the BottomDrawer's
  // property-editing UX. Each panel has its own mock state so playing with
  // one doesn't affect the others. No real API; entirely self-contained.

  type PropType = "select" | "date" | "text";
  type PropDef = {
    key: string;
    type: PropType;
    choices?: string[];
    chord?: string;             // single-char hot key for Spacemacs mode
    valueChords?: Record<string, string>; // value → chord letter
    bars?: boolean;             // render select value as bars
  };

  const PROPS: PropDef[] = [
    {
      key: "status",
      type: "select",
      chord: "s",
      choices: ["todo", "doing", "in-review", "done", "backlog", "canceled", "on-hold"],
      valueChords: { todo: "t", doing: "i", "in-review": "r", done: "D", backlog: "b", canceled: "c", "on-hold": "h" },
    },
    {
      key: "priority",
      type: "select",
      chord: "p",
      choices: ["low", "medium", "high"],
      valueChords: { low: "l", medium: "m", high: "h" },
      bars: true,
    },
    { key: "deadline", type: "date", chord: "d" },
    { key: "tags",     type: "text", chord: "t" },
  ];

  const INITIAL: Record<string, string> = {
    status: "doing",
    priority: "high",
    deadline: "2026-05-08",
    tags: "Task, Phase3GQA",
  };

  // Independent state per mockup so they don't cross-pollute.
  let valuesA = $state({ ...INITIAL });
  let valuesB = $state({ ...INITIAL });
  let valuesC = $state({ ...INITIAL });
  let valuesD = $state({ ...INITIAL });

  let activeMockup = $state(0);
  const TABS = [
    { id: 0, label: "A. Linear-style picker" },
    { id: 1, label: "B. Notion form" },
    { id: 2, label: "C. Spacemacs chord" },
    { id: 3, label: "D. Tag-grouped panel" },
  ];

  // Shared formatters
  function formatBars(value: string, choices: string[] | undefined): string {
    if (!choices) return value;
    const idx = choices.indexOf(value);
    if (idx < 0) return value;
    const filled = Math.max(1, Math.round(((idx + 1) / choices.length) * 3));
    return "▰".repeat(filled) + "▱".repeat(3 - filled);
  }

  function next<T>(arr: T[], cur: T): T {
    const i = arr.indexOf(cur);
    return arr[(i + 1) % arr.length];
  }

  // ────────────────────────────────────────────────────────────────────
  // A. LINEAR-STYLE INLINE PICKER
  // ────────────────────────────────────────────────────────────────────

  let aFocusIdx = $state(0);
  let aPickerOpen = $state(false);
  let aRoot = $state<HTMLDivElement | undefined>();

  function handleA(e: KeyboardEvent) {
    if (activeMockup !== 0) return;
    const prop = PROPS[aFocusIdx];
    if (!prop) return;

    if (aPickerOpen) {
      if (e.key === "Escape") { e.preventDefault(); aPickerOpen = false; return; }
      if (prop.type === "select" && prop.choices) {
        // Letter shortcut → set value
        const match = Object.entries(prop.valueChords ?? {}).find(([_, k]) => k === e.key);
        if (match) {
          e.preventDefault();
          valuesA[prop.key] = match[0];
          aPickerOpen = false;
        } else if (e.key === "j") {
          e.preventDefault();
          valuesA[prop.key] = next(prop.choices, valuesA[prop.key]);
        } else if (e.key === "k") {
          e.preventDefault();
          const i = prop.choices.indexOf(valuesA[prop.key]);
          valuesA[prop.key] = prop.choices[(i - 1 + prop.choices.length) % prop.choices.length];
        } else if (e.key === "Enter") {
          e.preventDefault();
          aPickerOpen = false;
        }
      }
      return;
    }

    if (e.key === "j" || e.key === "ArrowDown") { e.preventDefault(); aFocusIdx = Math.min(PROPS.length - 1, aFocusIdx + 1); }
    else if (e.key === "k" || e.key === "ArrowUp") { e.preventDefault(); aFocusIdx = Math.max(0, aFocusIdx - 1); }
    else if (e.key === "Enter" || e.key === " ") { e.preventDefault(); aPickerOpen = true; }
    else if (e.key === "x") { e.preventDefault(); valuesA[prop.key] = ""; }
  }

  // ────────────────────────────────────────────────────────────────────
  // B. NOTION-STYLE FORM
  // ────────────────────────────────────────────────────────────────────

  let bFocusIdx = $state(0);
  let bEditing = $state(false);
  let bRoot = $state<HTMLDivElement | undefined>();

  function handleB(e: KeyboardEvent) {
    if (activeMockup !== 1) return;
    const prop = PROPS[bFocusIdx];
    if (!prop) return;
    if (bEditing) return; // form-input owns keys
    if (e.key === "Tab") {
      e.preventDefault();
      bFocusIdx = e.shiftKey
        ? (bFocusIdx - 1 + PROPS.length) % PROPS.length
        : (bFocusIdx + 1) % PROPS.length;
    } else if (e.key === "j" || e.key === "ArrowDown") { e.preventDefault(); bFocusIdx = Math.min(PROPS.length - 1, bFocusIdx + 1); }
    else if (e.key === "k" || e.key === "ArrowUp") { e.preventDefault(); bFocusIdx = Math.max(0, bFocusIdx - 1); }
    else if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      if (prop.type === "select" && prop.choices) {
        valuesB[prop.key] = next(prop.choices, valuesB[prop.key]);
      } else {
        bEditing = true;
      }
    } else if (e.key === "x") { e.preventDefault(); valuesB[prop.key] = ""; }
  }

  // ────────────────────────────────────────────────────────────────────
  // C. SPACEMACS CHORD
  // ────────────────────────────────────────────────────────────────────

  let cChordPath = $state<"root" | "value">("root");
  let cActiveProp = $state<PropDef | null>(null);
  let cTextEdit = $state("");
  let cTextEditing = $state(false);
  let cRoot = $state<HTMLDivElement | undefined>();

  function handleC(e: KeyboardEvent) {
    if (activeMockup !== 2) return;
    if (cTextEditing) return;
    if (e.key === "Escape") {
      e.preventDefault();
      cChordPath = "root";
      cActiveProp = null;
      return;
    }
    if (cChordPath === "root") {
      const found = PROPS.find((p) => p.chord === e.key);
      if (found) {
        e.preventDefault();
        cActiveProp = found;
        if (found.type === "select") cChordPath = "value";
        else if (found.type === "text") {
          cTextEdit = valuesC[found.key] ?? "";
          cTextEditing = true;
        }
        else if (found.type === "date") {
          // For date: open native date input, focus it
          cTextEdit = valuesC[found.key] ?? "";
          cTextEditing = true;
        }
      }
    } else if (cChordPath === "value" && cActiveProp?.type === "select") {
      const match = Object.entries(cActiveProp.valueChords ?? {}).find(([_, k]) => k === e.key);
      if (match) {
        e.preventDefault();
        valuesC[cActiveProp.key] = match[0];
        cChordPath = "root";
        cActiveProp = null;
      }
    }
  }

  function commitTextC() {
    if (cActiveProp) {
      valuesC[cActiveProp.key] = cTextEdit;
    }
    cTextEditing = false;
    cChordPath = "root";
    cActiveProp = null;
    requestAnimationFrame(() => cRoot?.focus());
  }

  function cancelTextC() {
    cTextEditing = false;
    cChordPath = "root";
    cActiveProp = null;
    requestAnimationFrame(() => cRoot?.focus());
  }

  // ────────────────────────────────────────────────────────────────────
  // D. TAG-GROUPED PANEL
  // ────────────────────────────────────────────────────────────────────

  type Group = { tag: string; props: PropDef[]; collapsed: boolean };
  let dGroups = $state<Group[]>([
    { tag: "Task", collapsed: false, props: [PROPS[0], PROPS[1]] },                 // status, priority
    { tag: "Phase3GQA (inherited)", collapsed: false, props: [PROPS[2]] },          // deadline
    { tag: "All blocks", collapsed: false, props: [PROPS[3]] },                     // tags
  ]);
  let dFocus = $state<{ groupIdx: number; propIdx: number }>({ groupIdx: 0, propIdx: 0 });
  let dRoot = $state<HTMLDivElement | undefined>();

  function dFlatProps(): { groupIdx: number; propIdx: number; prop: PropDef }[] {
    const out: { groupIdx: number; propIdx: number; prop: PropDef }[] = [];
    dGroups.forEach((g, gi) => {
      if (!g.collapsed) g.props.forEach((p, pi) => out.push({ groupIdx: gi, propIdx: pi, prop: p }));
    });
    return out;
  }

  function dCurrentFlatIdx(): number {
    return dFlatProps().findIndex(
      (e) => e.groupIdx === dFocus.groupIdx && e.propIdx === dFocus.propIdx,
    );
  }

  function handleD(e: KeyboardEvent) {
    if (activeMockup !== 3) return;
    const flat = dFlatProps();
    const cur = dCurrentFlatIdx();
    const prop = flat[cur]?.prop;

    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      const next = flat[Math.min(flat.length - 1, cur + 1)];
      if (next) dFocus = { groupIdx: next.groupIdx, propIdx: next.propIdx };
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      const prev = flat[Math.max(0, cur - 1)];
      if (prev) dFocus = { groupIdx: prev.groupIdx, propIdx: prev.propIdx };
    } else if (e.key === "Tab") {
      e.preventDefault();
      // collapse/expand the group of the focused property
      dGroups[dFocus.groupIdx].collapsed = !dGroups[dFocus.groupIdx].collapsed;
      // If we just collapsed the current group, jump to first visible.
      if (dGroups[dFocus.groupIdx].collapsed) {
        const first = dFlatProps()[0];
        if (first) dFocus = { groupIdx: first.groupIdx, propIdx: first.propIdx };
      }
    } else if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      if (prop?.type === "select" && prop.choices) {
        valuesD[prop.key] = next(prop.choices, valuesD[prop.key]);
      }
    } else if (e.key === "x" && prop) {
      e.preventDefault();
      valuesD[prop.key] = "";
    }
  }
</script>

<svelte:head>
  <title>Tesela · Property editor mockups</title>
</svelte:head>

<div class="min-h-screen p-6" style="background: var(--background); color: var(--foreground)">
  <header class="mb-4">
    <h1 class="text-xl font-semibold mb-1">Property editor mockups</h1>
    <p class="text-sm" style="color: var(--muted-foreground)">
      Four interactive interaction-model prototypes for the BottomDrawer's Properties tab.
      Click into a panel and use its keyboard. Each panel has independent mock state.
    </p>
  </header>

  <!-- Tabs -->
  <div class="flex items-center gap-1 mb-4 border-b" style="border-color: var(--border)">
    {#each TABS as t}
      <button
        class="px-4 py-2 text-sm transition-colors border-b-2 -mb-px"
        style="border-color: {activeMockup === t.id ? 'var(--primary)' : 'transparent'}; color: {activeMockup === t.id ? 'var(--foreground)' : 'var(--muted-foreground)'}"
        onclick={() => (activeMockup = t.id)}
      >
        {t.label}
      </button>
    {/each}
  </div>

  <!-- Mockup A: Linear-style inline picker ────────────────────────── -->
  {#if activeMockup === 0}
    <div class="grid gap-4" style="grid-template-columns: 1fr 320px">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        bind:this={aRoot}
        tabindex="0"
        onkeydown={handleA}
        onclick={() => aRoot?.focus()}
        class="rounded-lg border p-6 outline-none focus:ring-2"
        style="background: var(--surface); border-color: var(--border); --tw-ring-color: color-mix(in srgb, var(--primary) 35%, transparent)"
      >
        <div class="mb-4 text-xs uppercase tracking-widest" style="color: var(--muted-foreground)">
          BLOCK · dudbar
        </div>
        {#each PROPS as prop, i}
          {@const isFocused = aFocusIdx === i}
          <div class="flex items-center gap-3 py-2 px-2 rounded relative"
               style="background: {isFocused ? 'color-mix(in srgb, var(--primary) 8%, transparent)' : 'transparent'}">
            <span class="w-6 text-xs" style="color: {isFocused ? 'var(--primary)' : 'transparent'}">▸</span>
            <span class="w-24 text-xs uppercase tracking-wide" style="color: var(--muted-foreground)">{prop.key}</span>
            <button
              class="text-sm px-2 py-0.5 rounded border flex items-center gap-1.5"
              style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
              onclick={() => { aFocusIdx = i; aPickerOpen = !aPickerOpen; }}
            >
              {#if prop.bars && valuesA[prop.key]}
                <span style="color: var(--primary)">{formatBars(valuesA[prop.key], prop.choices)}</span>
                <span style="color: var(--muted-foreground)">{valuesA[prop.key]}</span>
              {:else if valuesA[prop.key]}
                <span>{valuesA[prop.key]}</span>
              {:else}
                <span style="color: var(--muted-foreground)" class="italic">empty</span>
              {/if}
              <span class="text-[10px]" style="color: var(--muted-foreground)">▾</span>
            </button>
            <!-- inline picker -->
            {#if isFocused && aPickerOpen && prop.type === "select" && prop.choices}
              <div
                class="absolute left-32 top-9 z-10 rounded-md border shadow-lg p-1 min-w-[180px]"
                style="background: var(--popover); border-color: var(--border)"
              >
                {#each prop.choices as choice}
                  {@const chord = prop.valueChords?.[choice]}
                  {@const sel = valuesA[prop.key] === choice}
                  <button
                    class="w-full flex items-center gap-2 px-2 py-1 rounded text-left text-sm"
                    style="background: {sel ? 'color-mix(in srgb, var(--primary) 12%, transparent)' : 'transparent'}; color: var(--foreground)"
                    onclick={() => { valuesA[prop.key] = choice; aPickerOpen = false; aRoot?.focus(); }}
                  >
                    <kbd class="text-[10px] px-1.5 py-0 rounded border min-w-[18px] text-center"
                         style="background: var(--surface); border-color: var(--border); color: var(--primary)">{chord ?? "·"}</kbd>
                    <span class="flex-1">{choice}</span>
                    {#if sel}<span style="color: var(--primary)">✓</span>{/if}
                  </button>
                {/each}
                <div class="text-[10px] px-2 py-1 mt-1 border-t"
                     style="border-color: var(--border); color: var(--muted-foreground)">
                  letter to pick · j/k step · Esc cancel
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
      <aside class="rounded-lg border p-4 text-xs"
             style="background: var(--surface); border-color: var(--border)">
        <div class="text-xs uppercase tracking-widest mb-2" style="color: var(--muted-foreground)">Keys</div>
        <ul class="space-y-1">
          <li><kbd>j</kbd> / <kbd>k</kbd> · next / prev property</li>
          <li><kbd>Enter</kbd> or <kbd>Space</kbd> · open picker</li>
          <li><kbd>letter</kbd> · pick that choice (when picker open)</li>
          <li><kbd>x</kbd> · clear current</li>
          <li><kbd>Esc</kbd> · close picker</li>
        </ul>
      </aside>
    </div>
  {/if}

  <!-- Mockup B: Notion-style form ─────────────────────────────────── -->
  {#if activeMockup === 1}
    <div class="grid gap-4" style="grid-template-columns: 1fr 320px">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        bind:this={bRoot}
        tabindex="0"
        onkeydown={handleB}
        onclick={() => bRoot?.focus()}
        class="rounded-lg border p-6 outline-none focus:ring-2 max-w-xl"
        style="background: var(--surface); border-color: var(--border); --tw-ring-color: color-mix(in srgb, var(--primary) 35%, transparent)"
      >
        <div class="mb-6 text-base font-medium">dudbar</div>

        {#each PROPS as prop, i}
          {@const isFocused = bFocusIdx === i}
          <div class="grid items-center gap-4 py-3"
               style="grid-template-columns: 110px 1fr; background: {isFocused ? 'color-mix(in srgb, var(--primary) 6%, transparent)' : 'transparent'}; border-radius: 6px; padding-left: 8px">
            <label class="text-sm capitalize" style="color: var(--muted-foreground)">{prop.key}</label>
            {#if prop.type === "select" && prop.choices}
              <button
                class="text-sm text-left px-2 py-1.5 rounded border w-full max-w-[260px]"
                style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
                onclick={() => { bFocusIdx = i; valuesB[prop.key] = next(prop.choices!, valuesB[prop.key]); }}
              >
                {#if prop.bars && valuesB[prop.key]}
                  <span style="color: var(--primary)" class="mr-2">{formatBars(valuesB[prop.key], prop.choices)}</span>
                {/if}
                {valuesB[prop.key] || "—"}
              </button>
            {:else if prop.type === "date"}
              <input
                type="date"
                class="text-sm px-2 py-1.5 rounded border w-fit"
                style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
                bind:value={valuesB[prop.key]}
              />
            {:else}
              <input
                type="text"
                class="text-sm px-2 py-1.5 rounded border w-full max-w-[300px]"
                style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
                bind:value={valuesB[prop.key]}
              />
            {/if}
          </div>
        {/each}
        <div class="mt-4 pt-3" style="border-top: 1px solid var(--border)">
          <button class="text-sm px-2 py-1 rounded transition-colors"
                  style="color: var(--muted-foreground)">+ Add property</button>
        </div>
      </div>
      <aside class="rounded-lg border p-4 text-xs"
             style="background: var(--surface); border-color: var(--border)">
        <div class="text-xs uppercase tracking-widest mb-2" style="color: var(--muted-foreground)">Keys</div>
        <ul class="space-y-1">
          <li><kbd>Tab</kbd> / <kbd>S-Tab</kbd> · next / prev field</li>
          <li><kbd>j</kbd> / <kbd>k</kbd> · same as Tab</li>
          <li><kbd>Space</kbd> / <kbd>Enter</kbd> · cycle select / open editor</li>
          <li><kbd>x</kbd> · clear current</li>
        </ul>
      </aside>
    </div>
  {/if}

  <!-- Mockup C: Spacemacs chord ───────────────────────────────────── -->
  {#if activeMockup === 2}
    <div class="grid gap-4" style="grid-template-columns: 1fr 320px">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        bind:this={cRoot}
        tabindex="0"
        onkeydown={handleC}
        onclick={() => cRoot?.focus()}
        class="rounded-lg border p-6 outline-none focus:ring-2 min-h-[360px]"
        style="background: var(--surface); border-color: var(--border); --tw-ring-color: color-mix(in srgb, var(--primary) 35%, transparent)"
      >
        <div class="mb-6 text-xs uppercase tracking-widest" style="color: var(--muted-foreground)">
          BLOCK · dudbar · chord mode
        </div>

        {#if cTextEditing && cActiveProp}
          <div class="mt-2 max-w-md">
            <div class="text-xs mb-2" style="color: var(--muted-foreground)">edit {cActiveProp.key}</div>
            {#if cActiveProp.type === "date"}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                type="date"
                bind:value={cTextEdit}
                autofocus
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); commitTextC(); } else if (e.key === "Escape") { e.preventDefault(); cancelTextC(); } }}
                class="text-sm px-2 py-1.5 rounded border"
                style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
              />
            {:else}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                type="text"
                bind:value={cTextEdit}
                autofocus
                onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); commitTextC(); } else if (e.key === "Escape") { e.preventDefault(); cancelTextC(); } }}
                class="text-sm px-2 py-1.5 rounded border w-full max-w-md"
                style="background: var(--popover); border-color: var(--border); color: var(--foreground)"
              />
            {/if}
            <div class="text-[10px] mt-2" style="color: var(--muted-foreground)">Enter commit · Esc cancel</div>
          </div>
        {:else if cChordPath === "root"}
          <div class="space-y-1">
            {#each PROPS as prop}
              <div class="flex items-center gap-3 py-1.5 text-sm">
                <kbd class="text-[11px] px-1.5 py-0.5 rounded border min-w-[24px] text-center font-semibold"
                     style="background: var(--popover); border-color: var(--border); color: var(--primary)">{prop.chord}</kbd>
                <span class="w-24" style="color: var(--muted-foreground)">{prop.key}</span>
                <span style="color: var(--foreground)">
                  {#if prop.bars && valuesC[prop.key]}
                    <span style="color: var(--primary)">{formatBars(valuesC[prop.key], prop.choices)}</span>
                    <span style="color: var(--muted-foreground)" class="ml-1">{valuesC[prop.key]}</span>
                  {:else}
                    {valuesC[prop.key] || "—"}
                  {/if}
                </span>
              </div>
            {/each}
            <div class="text-[10px] pt-3 mt-3" style="color: var(--muted-foreground); border-top: 1px solid var(--border)">
              tap a property letter to enter, then a value letter to commit
            </div>
          </div>
        {:else if cChordPath === "value" && cActiveProp?.type === "select"}
          <div>
            <div class="mb-3 flex items-center gap-2 text-sm">
              <kbd class="text-[11px] px-1.5 py-0.5 rounded border font-semibold"
                   style="background: var(--popover); border-color: var(--border); color: var(--primary)">{cActiveProp.chord}</kbd>
              <span style="color: var(--foreground)">{cActiveProp.key}</span>
              <span style="color: var(--muted-foreground)">· pick a value</span>
            </div>
            <div class="space-y-1 ml-1">
              {#each cActiveProp.choices ?? [] as choice}
                {@const chord = cActiveProp.valueChords?.[choice]}
                {@const sel = valuesC[cActiveProp.key] === choice}
                <div class="flex items-center gap-3 py-1.5 text-sm">
                  <kbd class="text-[11px] px-1.5 py-0.5 rounded border min-w-[24px] text-center font-semibold"
                       style="background: var(--popover); border-color: var(--border); color: var(--primary)">{chord ?? "·"}</kbd>
                  <span class="w-32" style="color: var(--foreground)">{choice}</span>
                  {#if sel}<span style="color: var(--primary)">✓</span>{/if}
                </div>
              {/each}
            </div>
            <div class="text-[10px] pt-3 mt-3" style="color: var(--muted-foreground); border-top: 1px solid var(--border)">Esc back to property chord</div>
          </div>
        {/if}
      </div>
      <aside class="rounded-lg border p-4 text-xs"
             style="background: var(--surface); border-color: var(--border)">
        <div class="text-xs uppercase tracking-widest mb-2" style="color: var(--muted-foreground)">Keys</div>
        <ul class="space-y-1">
          <li><kbd>s</kbd> · status · then <kbd>t/i/r/D/b/c/h</kbd></li>
          <li><kbd>p</kbd> · priority · then <kbd>l/m/h</kbd></li>
          <li><kbd>d</kbd> · deadline · opens date input</li>
          <li><kbd>t</kbd> · tags · opens text input</li>
          <li><kbd>Esc</kbd> · back / cancel</li>
        </ul>
        <div class="text-[10px] mt-3" style="color: var(--muted-foreground)">
          Two keystrokes for any select edit. <br/>
          Like vim's <code>r&lt;char&gt;</code>.
        </div>
      </aside>
    </div>
  {/if}

  <!-- Mockup D: Tag-grouped panel ─────────────────────────────────── -->
  {#if activeMockup === 3}
    <div class="grid gap-4" style="grid-template-columns: 1fr 320px">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        bind:this={dRoot}
        tabindex="0"
        onkeydown={handleD}
        onclick={() => dRoot?.focus()}
        class="rounded-lg border p-6 outline-none focus:ring-2"
        style="background: var(--surface); border-color: var(--border); --tw-ring-color: color-mix(in srgb, var(--primary) 35%, transparent)"
      >
        <div class="mb-4 text-xs uppercase tracking-widest" style="color: var(--muted-foreground)">
          BLOCK · dudbar
        </div>

        {#each dGroups as group, gi}
          <div class="mb-4">
            <div class="flex items-center gap-2 text-xs mb-1.5 uppercase tracking-wide"
                 style="color: var(--muted-foreground)">
              <span class="text-[10px]">{group.collapsed ? "▶" : "▼"}</span>
              <span style="color: var(--foreground)">{group.tag}</span>
            </div>
            {#if !group.collapsed}
              {#each group.props as prop, pi}
                {@const isFocused = dFocus.groupIdx === gi && dFocus.propIdx === pi}
                <div class="flex items-center gap-3 py-1.5 px-2 ml-3 rounded text-sm"
                     style="background: {isFocused ? 'color-mix(in srgb, var(--primary) 8%, transparent)' : 'transparent'}">
                  <span class="w-4 text-xs" style="color: {isFocused ? 'var(--primary)' : 'transparent'}">▸</span>
                  <span class="w-24" style="color: var(--muted-foreground)">{prop.key}</span>
                  <span style="color: var(--foreground)">
                    {#if prop.bars && valuesD[prop.key]}
                      <span style="color: var(--primary)">{formatBars(valuesD[prop.key], prop.choices)}</span>
                      <span style="color: var(--muted-foreground)" class="ml-1">{valuesD[prop.key]}</span>
                    {:else}
                      {valuesD[prop.key] || "—"}
                    {/if}
                  </span>
                </div>
              {/each}
            {/if}
          </div>
        {/each}
      </div>
      <aside class="rounded-lg border p-4 text-xs"
             style="background: var(--surface); border-color: var(--border)">
        <div class="text-xs uppercase tracking-widest mb-2" style="color: var(--muted-foreground)">Keys</div>
        <ul class="space-y-1">
          <li><kbd>j</kbd> / <kbd>k</kbd> · next / prev (across groups)</li>
          <li><kbd>Tab</kbd> · collapse / expand current group</li>
          <li><kbd>Space</kbd> / <kbd>Enter</kbd> · cycle select</li>
          <li><kbd>x</kbd> · clear current</li>
        </ul>
      </aside>
    </div>
  {/if}

  <!-- Footer -->
  <footer class="mt-8 text-xs" style="color: var(--muted-foreground)">
    Mock data only. Click a panel and use its keyboard. State per panel is independent.
  </footer>
</div>

<style>
  kbd {
    display: inline-block;
    padding: 1px 6px;
    background: var(--popover);
    border: 1px solid var(--border);
    border-radius: 3px;
    font-family: var(--v9-mono, monospace);
    font-size: 10px;
    color: var(--primary);
  }
</style>
