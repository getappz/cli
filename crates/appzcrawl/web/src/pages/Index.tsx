import {
  Copy,
  Eye,
  EyeOff,
  Grip,
  Maximize2,
  Search,
  Sparkles,
  Terminal,
} from "lucide-react";
import { useState } from "react";
import {
  Area,
  AreaChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import DashboardLayout from "@/components/DashboardLayout";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { useToast } from "@/hooks/use-toast";

const endpoints = [
  {
    icon: Grip,
    title: "Scrape",
    desc: "Get llm-ready data from websites. Markdown, JSON, screenshot, etc.",
  },
  {
    icon: Search,
    title: "Search",
    desc: "Search the web and get full content from results.",
  },
  {
    icon: Maximize2,
    title: "Crawl",
    desc: "Crawl all the pages on a website and get data for each page.",
  },
  {
    icon: Sparkles,
    title: "Agent",
    desc: "Gather data wherever it lives on the web.",
    badge: "NEW",
  },
];

const chartData = [
  { date: "02/06", pages: 12 },
  { date: "02/07", pages: 18 },
  { date: "02/08", pages: 45 },
  { date: "02/09", pages: 68 },
  { date: "02/10", pages: 52 },
  { date: "02/11", pages: 30 },
  { date: "02/12", pages: 15 },
  { date: "02/13", pages: 8 },
];

const totalPages = 105;
const maskedKey = "fc-5•••••••••••••••••••••••942b";
const fullKey = "fc-5a1b2c3d4e5f6g7h8i9j0k1l2m3n942b";

const Index = () => {
  const [showKey, setShowKey] = useState(false);
  const { toast } = useToast();

  const copyKey = () => {
    navigator.clipboard.writeText(fullKey);
    toast({ title: "Copied!", description: "API key copied to clipboard" });
  };

  return (
    <DashboardLayout>
      <div className="space-y-8">
        {/* Explore Endpoints */}
        <div>
          <h1 className="text-2xl font-semibold text-foreground">
            Explore our endpoints
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Power your applications with our comprehensive scraping API
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {endpoints.map((ep) => (
            <Card
              key={ep.title}
              className="p-5 bg-card border-border hover:border-primary/30 transition-colors cursor-pointer group"
            >
              <div className="flex items-start gap-3">
                <div className="p-1.5 rounded bg-primary/5 text-primary">
                  <ep.icon className="h-4 w-4" />
                </div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <h3 className="text-base font-semibold text-foreground">
                      {ep.title}
                    </h3>
                    {ep.badge && (
                      <Badge className="bg-primary/10 text-primary border-primary/20 text-[10px] px-1.5 py-0">
                        {ep.badge}
                      </Badge>
                    )}
                  </div>
                  <p className="text-sm text-muted-foreground mt-1 leading-relaxed">
                    {ep.desc}
                  </p>
                </div>
              </div>
            </Card>
          ))}
        </div>

        {/* Scraped Pages + API Key Row */}
        <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
          {/* Scraped Pages Chart */}
          <Card className="lg:col-span-3 p-6 bg-card border-border">
            <div className="flex items-start justify-between mb-1">
              <div>
                <h2 className="text-lg font-semibold text-foreground">
                  Scraped pages - Last 7 days
                </h2>
                <p className="text-sm text-muted-foreground">
                  Credit usage differs
                </p>
              </div>
              <span className="text-4xl font-semibold text-foreground">
                {totalPages}
              </span>
            </div>
            <div className="h-52 mt-4">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData}>
                  <defs>
                    <linearGradient id="colorPages" x1="0" y1="0" x2="0" y2="1">
                      <stop
                        offset="5%"
                        stopColor="hsl(24, 100%, 50%)"
                        stopOpacity={0.15}
                      />
                      <stop
                        offset="95%"
                        stopColor="hsl(24, 100%, 50%)"
                        stopOpacity={0}
                      />
                    </linearGradient>
                  </defs>
                  <XAxis
                    dataKey="date"
                    tick={{ fill: "hsl(0, 0%, 45%)", fontSize: 12 }}
                    axisLine={{ stroke: "hsl(30, 10%, 90%)" }}
                    tickLine={false}
                  />
                  <YAxis hide />
                  <Tooltip
                    contentStyle={{
                      background: "hsl(0, 0%, 100%)",
                      border: "1px solid hsl(30, 10%, 90%)",
                      borderRadius: "8px",
                      color: "hsl(0, 0%, 9%)",
                      fontSize: 13,
                    }}
                  />
                  <Area
                    type="monotone"
                    dataKey="pages"
                    stroke="hsl(24, 100%, 50%)"
                    strokeWidth={2}
                    fillOpacity={1}
                    fill="url(#colorPages)"
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </Card>

          {/* Right column: API Key + Agent Integrations */}
          <div className="lg:col-span-2 space-y-6">
            {/* API Key */}
            <Card className="p-6 bg-card border-border">
              <h2 className="text-lg font-semibold text-foreground">API Key</h2>
              <p className="text-sm text-muted-foreground mb-4">
                Start scraping right away
              </p>
              <div className="flex items-center gap-2 rounded-lg border border-dashed border-primary/40 bg-primary/5 px-4 py-2.5">
                <code className="flex-1 text-sm font-mono text-primary tracking-wide">
                  {showKey ? fullKey : maskedKey}
                </code>
                <button
                  type="button"
                  onClick={() => setShowKey(!showKey)}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  {showKey ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
                <button
                  type="button"
                  onClick={copyKey}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  <Copy className="h-4 w-4" />
                </button>
              </div>
            </Card>

            {/* Agent Integrations */}
            <Card className="p-6 bg-card border-border">
              <div className="flex items-start justify-between">
                <div>
                  <h2 className="text-lg font-semibold text-foreground">
                    Agent Integrations
                  </h2>
                  <p className="text-sm text-muted-foreground">
                    Give your AI agents web data
                  </p>
                </div>
                <div className="flex gap-2">
                  <div className="h-7 w-7 rounded bg-accent flex items-center justify-center">
                    <Sparkles className="h-4 w-4 text-muted-foreground" />
                  </div>
                  <div className="h-7 w-7 rounded bg-accent flex items-center justify-center">
                    <span className="text-xs">🤖</span>
                  </div>
                  <div className="h-7 w-7 rounded bg-accent flex items-center justify-center">
                    <span className="text-xs font-bold text-muted-foreground">
                      W
                    </span>
                  </div>
                </div>
              </div>

              <div className="mt-4 pt-4 border-t border-border">
                <p className="text-sm font-medium text-foreground">
                  Skills + CLI
                </p>
                <div className="flex items-center gap-2 mt-2 rounded-lg bg-muted px-3 py-2">
                  <Terminal className="h-3.5 w-3.5 text-muted-foreground" />
                  <code className="text-sm font-mono text-foreground flex-1">
                    $ npx skills add{" "}
                    <span className="text-primary">firecrawl/cli</span>
                  </code>
                  <button
                    type="button"
                    onClick={() => {
                      navigator.clipboard.writeText(
                        "npx skills add firecrawl/cli",
                      );
                      toast({ title: "Copied!" });
                    }}
                    className="text-muted-foreground hover:text-foreground transition-colors"
                  >
                    <Copy className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
            </Card>
          </div>
        </div>
      </div>
    </DashboardLayout>
  );
};

export default Index;
