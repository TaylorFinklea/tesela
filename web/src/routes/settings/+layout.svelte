<script lang="ts">
  import { page } from "$app/state";
  let { children } = $props();

  const tabs = [
    { href: "/settings/general", label: "General", hint: "Editor, fonts, server, keyboard" },
    { href: "/settings/mosaic", label: "Mosaic", hint: "Create or switch mosaic" },
    { href: "/settings/data", label: "Data", hint: "Backups, export, import" },
    { href: "/settings/sync", label: "Sync", hint: "Notifications, Apple Reminders" },
  ];
</script>

<div class="flex-1 flex flex-col min-h-0">
  <header class="border-b border-border px-5 h-11 flex items-center shrink-0">
    <h1 class="text-[13px] font-semibold tracking-tight">Settings</h1>
  </header>

  <div class="flex-1 flex min-h-0">
    <nav class="w-44 border-r border-border/40 shrink-0 py-3 px-2 space-y-0.5">
      {#each tabs as t}
        {@const active = page.url.pathname.startsWith(t.href)}
        <a
          href={t.href}
          class="block rounded-md px-2.5 py-1.5 transition-colors {active ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-muted/40 hover:text-foreground'}"
        >
          <div class="text-[12px] font-medium">{t.label}</div>
          <div class="text-[10px] opacity-60 leading-tight mt-0.5">{t.hint}</div>
        </a>
      {/each}
    </nav>

    <div class="flex-1 overflow-y-auto min-h-0">
      <div class="max-w-lg mx-auto py-8 px-6 space-y-8">
        {@render children()}
      </div>
    </div>
  </div>
</div>
