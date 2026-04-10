"use client";

import {
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { wsClient } from "@/lib/ws-client";
import { CommandPalette } from "@/components/CommandPalette";

/**
 * Client-side provider tree: TanStack Query + WsClient + Command Palette.
 */
export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            // Local server is fast; refetch on focus is not useful when the
            // WebSocket already pushes changes.
            refetchOnWindowFocus: false,
            // 30s is plenty — WS invalidation will usually beat staleness anyway.
            staleTime: 30_000,
          },
        },
      }),
  );

  useEffect(() => {
    wsClient.connect();
    // Intentionally do not disconnect on unmount: React StrictMode's double-mount
    // would otherwise thrash the connection. WsClient.connect() is idempotent.
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      {children}
      <CommandPalette />
    </QueryClientProvider>
  );
}
