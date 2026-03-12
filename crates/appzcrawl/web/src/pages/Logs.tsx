import { CheckCircle, Clock, Loader2, XCircle } from "lucide-react";
import DashboardLayout from "@/components/DashboardLayout";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";

const logs = [
  {
    id: "req_abc123",
    endpoint: "POST /v1/scrape",
    url: "https://docs.stripe.com",
    status: 200,
    duration: "1.4s",
    credits: 1,
    time: "2025-02-12 14:32:01",
    state: "success",
  },
  {
    id: "req_def456",
    endpoint: "POST /v1/crawl",
    url: "https://example.com",
    status: 200,
    duration: "12.8s",
    credits: 15,
    time: "2025-02-12 14:28:45",
    state: "success",
  },
  {
    id: "req_ghi789",
    endpoint: "POST /v1/search",
    url: "AI scraping tools 2025",
    status: 200,
    duration: "0.9s",
    credits: 1,
    time: "2025-02-12 14:25:12",
    state: "success",
  },
  {
    id: "req_jkl012",
    endpoint: "POST /v1/scrape",
    url: "https://invalid-url.xyz",
    status: 422,
    duration: "0.3s",
    credits: 0,
    time: "2025-02-12 14:20:33",
    state: "failed",
  },
  {
    id: "req_mno345",
    endpoint: "POST /v1/crawl",
    url: "https://blog.example.com",
    status: 200,
    duration: "—",
    credits: 0,
    time: "2025-02-12 14:18:09",
    state: "running",
  },
  {
    id: "req_pqr678",
    endpoint: "POST /v1/map",
    url: "https://firecrawl.dev",
    status: 200,
    duration: "0.2s",
    credits: 1,
    time: "2025-02-12 14:15:44",
    state: "success",
  },
  {
    id: "req_stu901",
    endpoint: "POST /v1/scrape",
    url: "https://github.com/firecrawl",
    status: 200,
    duration: "2.1s",
    credits: 1,
    time: "2025-02-12 14:10:22",
    state: "success",
  },
  {
    id: "req_vwx234",
    endpoint: "POST /v1/scrape",
    url: "https://news.ycombinator.com",
    status: 408,
    duration: "30.0s",
    credits: 0,
    time: "2025-02-12 14:05:11",
    state: "failed",
  },
];

const StateIcon = ({ state }: { state: string }) => {
  switch (state) {
    case "success":
      return <CheckCircle className="h-4 w-4 text-success" />;
    case "failed":
      return <XCircle className="h-4 w-4 text-destructive" />;
    case "running":
      return <Loader2 className="h-4 w-4 text-warning animate-spin" />;
    default:
      return <Clock className="h-4 w-4 text-muted-foreground" />;
  }
};

const Logs = () => {
  return (
    <DashboardLayout>
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">
            Request Logs
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            View your recent API requests
          </p>
        </div>

        <Card className="bg-card border-border overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-secondary/50">
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Status
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Request ID
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Endpoint
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    URL / Query
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Code
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Duration
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Credits
                  </th>
                  <th className="text-left px-4 py-3 text-muted-foreground font-medium">
                    Time
                  </th>
                </tr>
              </thead>
              <tbody>
                {logs.map((log) => (
                  <tr
                    key={log.id}
                    className="border-b border-border hover:bg-secondary/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <StateIcon state={log.state} />
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
                      {log.id}
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-foreground">
                      {log.endpoint}
                    </td>
                    <td className="px-4 py-3 text-xs text-muted-foreground max-w-[200px] truncate">
                      {log.url}
                    </td>
                    <td className="px-4 py-3">
                      <Badge
                        variant={log.status < 400 ? "default" : "destructive"}
                        className="text-xs font-mono"
                      >
                        {log.status}
                      </Badge>
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
                      {log.duration}
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-foreground">
                      {log.credits}
                    </td>
                    <td className="px-4 py-3 text-xs text-muted-foreground whitespace-nowrap">
                      {log.time}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      </div>
    </DashboardLayout>
  );
};

export default Logs;
