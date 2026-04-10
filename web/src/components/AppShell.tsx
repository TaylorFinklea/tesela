import { useState } from "react";
import { Sidebar } from "./Sidebar";

/**
 * App shell with collapsible sidebar + main content area.
 * Wraps the entire app — used in providers.tsx.
 */
export function AppShell({ children }: { children: React.ReactNode }) {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  return (
    <div className="flex h-full">
      <Sidebar
        collapsed={sidebarCollapsed}
        onToggle={() => setSidebarCollapsed((prev) => !prev)}
      />
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {children}
      </main>
    </div>
  );
}
