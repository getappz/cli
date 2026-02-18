import { Bell, CircleHelp, FileText, MessageSquare, Zap } from "lucide-react";
import { Button } from "@/components/ui/button";

const TopBar = () => {
  return (
    <header className="sticky top-0 z-30 flex h-14 items-center justify-between border-b border-border bg-background px-6">
      {/* Left - Team selector */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="flex items-center gap-2 rounded-full border border-border px-4 py-1.5 text-sm font-medium text-foreground hover:bg-accent transition-colors"
        >
          <span className="h-5 w-5 rounded bg-primary/10 text-primary flex items-center justify-center text-[10px] font-bold">
            P
          </span>
          Personal Team
          <svg
            aria-hidden="true"
            className="h-3 w-3 text-muted-foreground"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>
      </div>

      {/* Right - Actions */}
      <div className="flex items-center gap-1">
        <button
          type="button"
          className="relative p-2 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <Bell className="h-5 w-5" />
          <span className="absolute top-1 right-1 h-4 w-4 rounded-full bg-primary text-primary-foreground text-[10px] font-bold flex items-center justify-center">
            1
          </span>
        </button>
        <button
          type="button"
          className="p-2 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <MessageSquare className="h-5 w-5" />
        </button>
        <button
          type="button"
          className="flex items-center gap-1.5 px-3 py-2 rounded-md text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <CircleHelp className="h-4 w-4" />
          Help
        </button>
        <button
          type="button"
          className="flex items-center gap-1.5 px-3 py-2 rounded-md text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <FileText className="h-4 w-4" />
          Docs
        </button>
        <Button
          type="button"
          size="sm"
          className="gradient-orange text-primary-foreground ml-2 gap-1.5 rounded-full px-4"
        >
          <Zap className="h-3.5 w-3.5" />
          Upgrade
        </Button>
      </div>
    </header>
  );
};

export default TopBar;
