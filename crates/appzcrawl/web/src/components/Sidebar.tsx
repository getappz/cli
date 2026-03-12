import {
  BarChart3,
  ChevronLeft,
  Flame,
  Key,
  LayoutDashboard,
  Megaphone,
  Play,
  ScrollText,
  Search,
  Settings,
  Sparkles,
} from "lucide-react";
import { NavLink } from "react-router-dom";
import { Badge } from "@/components/ui/badge";

const navItems = [
  { to: "/agent", icon: Sparkles, label: "Agent", badge: "PREVIEW" },
  { to: "/", icon: LayoutDashboard, label: "Overview" },
  { to: "/playground", icon: Play, label: "Playground" },
  { to: "/logs", icon: ScrollText, label: "Activity Logs" },
  { to: "/usage", icon: BarChart3, label: "Usage" },
  { to: "/api-keys", icon: Key, label: "API Keys" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

const Sidebar = () => {
  return (
    <aside className="fixed left-0 top-0 z-40 flex h-screen w-[240px] flex-col border-r border-sidebar-border bg-sidebar">
      {/* Logo */}
      <div className="flex items-center gap-2 px-5 py-4">
        <Flame className="h-6 w-6 text-primary" />
        <span className="text-base font-semibold text-foreground tracking-tight">
          Firecrawl
        </span>
      </div>

      {/* Search */}
      <div className="px-3 pb-2">
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-md border border-border bg-background px-3 py-1.5 text-sm text-muted-foreground hover:bg-accent transition-colors"
        >
          <Search className="h-3.5 w-3.5" />
          <span className="flex-1 text-left">Search</span>
          <kbd className="text-xs text-muted-foreground border border-border rounded px-1.5 py-0.5 bg-muted">
            ⌘K
          </kbd>
        </button>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-3 py-1 space-y-0.5">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-sidebar-foreground hover:bg-accent hover:text-foreground"
              }`
            }
          >
            <item.icon className="h-4 w-4" />
            <span className="flex-1">{item.label}</span>
            {item.badge && (
              <Badge className="bg-primary/10 text-primary border-0 text-[10px] px-1.5 py-0 font-semibold">
                {item.badge}
              </Badge>
            )}
          </NavLink>
        ))}
      </nav>

      {/* What's New Card */}
      <div className="px-3 pb-2">
        <div className="rounded-lg bg-sidebar-accent p-3 flex items-start gap-3">
          <Megaphone className="h-5 w-5 text-primary mt-0.5" />
          <div>
            <p className="text-sm font-medium text-foreground">What's New</p>
            <p className="text-xs text-muted-foreground">
              View our latest update
            </p>
          </div>
        </div>
      </div>

      {/* User */}
      <div className="border-t border-sidebar-border px-3 py-3 space-y-2">
        <div className="flex items-center gap-3 px-2">
          <div className="h-7 w-7 rounded-full bg-secondary flex items-center justify-center text-xs font-semibold text-secondary-foreground">
            SK
          </div>
          <span className="text-sm text-foreground truncate">
            kumar@adsonmedia.com
          </span>
        </div>
        <button
          type="button"
          className="flex items-center gap-2 px-2 py-1 text-sm text-muted-foreground hover:text-foreground transition-colors w-full"
        >
          <ChevronLeft className="h-4 w-4" />
          <span>Collapse</span>
        </button>
      </div>
    </aside>
  );
};

export default Sidebar;
