<script lang="ts">
  import { onMount } from "svelte";
  import {
    checkboxIsChecked,
    isMultiSelectType,
    multiSelectDelta,
    parseMultiSelectValue,
    toggledCheckboxValue,
    type MultiSelectDelta,
  } from "$lib/property-editing";

  let {
    propertyName,
    currentValue,
    valueType,
    choices,
    position,
    onselect,
    onlistchange = undefined,
    onclose,
  }: {
    propertyName: string;
    currentValue: string;
    valueType: string;
    choices: string[] | null;
    position: { x: number; y: number };
    onselect: (value: string) => void;
    onlistchange?: (delta: MultiSelectDelta) => void;
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);
  let textValue = $state("");
  let selectedValues = $state<string[]>([]);
  const isMultiSelect = $derived(isMultiSelectType(valueType));

  // For select types, find current selection
  $effect(() => {
    if (choices && currentValue) {
      const idx = choices.indexOf(currentValue);
      if (idx >= 0) selectedIndex = idx;
    }
    textValue = currentValue;
    selectedValues = parseMultiSelectValue(currentValue);
  });

  function toggleMultiSelect(choice: string): void {
    selectedValues = selectedValues.includes(choice)
      ? selectedValues.filter((value) => value !== choice)
      : [...selectedValues, choice];
  }

  function saveMultiSelect(): void {
    const delta = multiSelectDelta(currentValue, selectedValues);
    if (onlistchange) onlistchange(delta);
    else onselect(selectedValues.join(", "));
  }

  function inputType(type: string): string {
    switch (type) {
      case "date": return "date";
      case "datetime": return "datetime-local";
      case "number": return "number";
      case "url": return "url";
      case "email": return "email";
      case "phone": return "tel";
      default: return "text";
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (isMultiSelect) {
      if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        saveMultiSelect();
      }
    } else if (valueType === "select" && choices) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectedIndex = Math.min(choices.length - 1, selectedIndex + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        selectedIndex = Math.max(0, selectedIndex - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        onselect(choices[selectedIndex]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      }
    } else if (valueType === "checkbox") {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onselect(toggledCheckboxValue(currentValue));
      } else if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      }
    } else {
      if (e.key === "Enter") {
        e.preventDefault();
        onselect(textValue);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      }
    }
  }

  onMount(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".property-editor")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="property-editor fixed z-50 rounded-md border border-border bg-popover text-popover-foreground shadow-lg w-48"
  style="left: {position.x}px; top: {position.y}px"
  onkeydown={handleKeydown}
>
  <div class="px-2 py-1 border-b border-border">
    <span class="text-[10px] text-muted-foreground/60 uppercase tracking-widest">{propertyName}</span>
  </div>

  {#if isMultiSelect && choices}
    <div class="py-1 max-h-52 overflow-y-auto" aria-label="{propertyName} choices">
      {#each choices as choice (choice)}
        <button
          type="button"
          class="w-full px-2 py-1.5 text-[12px] flex items-center gap-2 text-left hover:bg-accent"
          class:bg-accent={selectedValues.includes(choice)}
          aria-pressed={selectedValues.includes(choice)}
          onclick={() => toggleMultiSelect(choice)}
        >
          <span
            class="inline-flex h-3.5 w-3.5 items-center justify-center rounded-sm border border-border text-[10px]"
            class:bg-primary={selectedValues.includes(choice)}
            class:text-primary-foreground={selectedValues.includes(choice)}
          >{selectedValues.includes(choice) ? "✓" : ""}</span>
          <span>{choice}</span>
        </button>
      {/each}
    </div>
    <div class="flex justify-end gap-1 border-t border-border p-1.5">
      <button type="button" class="rounded px-2 py-1 text-[11px] hover:bg-muted" onclick={onclose}>Cancel</button>
      <button type="button" class="rounded bg-primary px-2 py-1 text-[11px] text-primary-foreground" onclick={saveMultiSelect}>Save</button>
    </div>
  {:else if valueType === "select" && choices}
    <div class="py-0.5 max-h-40 overflow-y-auto">
      {#each choices as choice, i}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="px-2 py-1 text-[12px] cursor-pointer flex items-center gap-2
            {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}
            {choice === currentValue ? 'font-medium' : ''}"
          onclick={() => onselect(choice)}
          onmouseenter={() => (selectedIndex = i)}
        >
          {#if choice === currentValue}
            <span class="text-primary text-[10px]">●</span>
          {:else}
            <span class="text-[10px] opacity-0">●</span>
          {/if}
          <span>{choice}</span>
        </div>
      {/each}
    </div>
  {:else if valueType === "checkbox"}
    <div class="p-2">
      <!-- svelte-ignore a11y_autofocus -->
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-[12px] hover:bg-muted"
        aria-pressed={checkboxIsChecked(currentValue)}
        onclick={() => onselect(toggledCheckboxValue(currentValue))}
        autofocus
      >
        <span class="text-base">{checkboxIsChecked(currentValue) ? "☑" : "☐"}</span>
        <span>{checkboxIsChecked(currentValue) ? "Checked" : "Unchecked"}</span>
      </button>
    </div>
  {:else}
    <div class="p-2">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type={inputType(valueType)}
        bind:value={textValue}
        onkeydown={handleKeydown}
        class="w-full text-[12px] bg-muted/50 rounded px-2 py-1 text-foreground outline-none border border-transparent focus:border-ring/30"
        autofocus
      />
    </div>
  {/if}
</div>
