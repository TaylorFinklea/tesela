<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";

  // Redirect to today's daily note
  onMount(async () => {
    try {
      const note = await api.getDailyNote();
      goto(`/p/${encodeURIComponent(note.id)}`, { replaceState: true });
    } catch (e) {
      console.error("Failed to get daily note:", e);
    }
  });
</script>

<div class="flex-1 flex items-center justify-center text-sm text-muted-foreground">
  Loading today's note…
</div>
