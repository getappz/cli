import { Copy, Eye, EyeOff, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useToast } from "@/hooks/use-toast";

interface ApiKey {
  id: string;
  name: string;
  key: string;
  created: string;
  lastUsed: string;
}

const initialKeys: ApiKey[] = [
  {
    id: "1",
    name: "Production",
    key: "fc-a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
    created: "Jan 15, 2025",
    lastUsed: "2 hours ago",
  },
  {
    id: "2",
    name: "Development",
    key: "fc-z9y8x7w6v5u4t3s2r1q0p9o8n7m6l5k4",
    created: "Feb 1, 2025",
    lastUsed: "5 min ago",
  },
];

const ApiKeys = () => {
  const [keys, setKeys] = useState<ApiKey[]>(initialKeys);
  const [visibleKeys, setVisibleKeys] = useState<Set<string>>(new Set());
  const [newKeyName, setNewKeyName] = useState("");
  const { toast } = useToast();

  const toggleVisibility = (id: string) => {
    setVisibleKeys((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const copyKey = (key: string) => {
    navigator.clipboard.writeText(key);
    toast({ title: "Copied!", description: "API key copied to clipboard" });
  };

  const createKey = () => {
    if (!newKeyName.trim()) return;
    const newKey: ApiKey = {
      id: Date.now().toString(),
      name: newKeyName,
      key: `fc-${Array.from({ length: 32 }, () => "abcdefghijklmnopqrstuvwxyz0123456789"[Math.floor(Math.random() * 36)]).join("")}`,
      created: "Just now",
      lastUsed: "Never",
    };
    setKeys((prev) => [...prev, newKey]);
    setNewKeyName("");
    toast({
      title: "Key Created",
      description: `API key "${newKeyName}" created successfully`,
    });
  };

  const deleteKey = (id: string) => {
    setKeys((prev) => prev.filter((k) => k.id !== id));
    toast({ title: "Key Deleted", description: "API key has been revoked" });
  };

  const maskKey = (key: string) =>
    key.slice(0, 5) + "•".repeat(24) + key.slice(-4);

  return (
    <DashboardLayout>
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">API Keys</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Manage your API keys for accessing the Firecrawl API
          </p>
        </div>

        {/* Create Key */}
        <Card className="bg-card border-border p-5">
          <h2 className="text-sm font-medium text-foreground mb-3">
            Create New Key
          </h2>
          <div className="flex gap-3">
            <Input
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
              placeholder="Key name (e.g., Production)"
              className="bg-secondary border-border text-foreground max-w-sm"
              onKeyDown={(e) => e.key === "Enter" && createKey()}
            />
            <Button
              onClick={createKey}
              className="gradient-orange text-primary-foreground gap-2"
            >
              <Plus className="h-4 w-4" />
              Create Key
            </Button>
          </div>
        </Card>

        {/* Keys List */}
        <div className="space-y-3">
          {keys.map((apiKey) => (
            <Card key={apiKey.id} className="bg-card border-border p-5">
              <div className="flex items-center justify-between">
                <div className="space-y-2">
                  <div className="flex items-center gap-3">
                    <h3 className="text-sm font-medium text-foreground">
                      {apiKey.name}
                    </h3>
                    <Badge variant="secondary" className="text-xs">
                      Active
                    </Badge>
                  </div>
                  <div className="flex items-center gap-2">
                    <code className="text-sm font-mono text-muted-foreground">
                      {visibleKeys.has(apiKey.id)
                        ? apiKey.key
                        : maskKey(apiKey.key)}
                    </code>
                    <button
                      type="button"
                      onClick={() => toggleVisibility(apiKey.id)}
                      className="text-muted-foreground hover:text-foreground transition-colors"
                    >
                      {visibleKeys.has(apiKey.id) ? (
                        <EyeOff className="h-4 w-4" />
                      ) : (
                        <Eye className="h-4 w-4" />
                      )}
                    </button>
                    <button
                      type="button"
                      onClick={() => copyKey(apiKey.key)}
                      className="text-muted-foreground hover:text-foreground transition-colors"
                    >
                      <Copy className="h-4 w-4" />
                    </button>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Created {apiKey.created} · Last used {apiKey.lastUsed}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => deleteKey(apiKey.id)}
                  className="text-muted-foreground hover:text-destructive"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            </Card>
          ))}
        </div>
      </div>
    </DashboardLayout>
  );
};

export default ApiKeys;
