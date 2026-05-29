<!-- web/src/lib/graphite/GrIcon.svelte -->
<script lang="ts">
  import {
    IconMicrophone, IconChartDots3 as IconGraph, IconSettings, IconSearch, IconPlus,
    IconBolt, IconPin, IconSun, IconSquareCheck, IconChevronDown, IconChevronRight,
    IconFlame, IconFolder, IconLayoutSidebar, IconAdjustments, IconHash, IconClock,
    IconCornerDownRight, IconInbox, IconCalendar, IconCircleDot, IconUser, IconLink,
    IconFileText, IconDotsVertical, IconArrowLeft
  } from '@tabler/icons-svelte';

  const MAP: Record<string, any> = {
    microphone: IconMicrophone, graph: IconGraph, settings: IconSettings, search: IconSearch,
    plus: IconPlus, bolt: IconBolt, pin: IconPin, sun: IconSun, 'square-check': IconSquareCheck,
    'chevron-down': IconChevronDown, 'chevron-right': IconChevronRight, flame: IconFlame,
    folder: IconFolder, 'layout-sidebar': IconLayoutSidebar, adjustments: IconAdjustments,
    hash: IconHash, clock: IconClock, 'corner-down-right': IconCornerDownRight, inbox: IconInbox,
    calendar: IconCalendar, 'circle-dot': IconCircleDot, user: IconUser, link: IconLink,
    'file-text': IconFileText, 'dots-vertical': IconDotsVertical, 'arrow-left': IconArrowLeft
  };
  let { name, size = 16, stroke = 1.75 }: { name: string; size?: number; stroke?: number } = $props();
  const Cmp = $derived(MAP[name]);
</script>

{#if Cmp}<Cmp {size} {stroke} />{/if}
