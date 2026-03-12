import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import DashboardLayout from "@/components/DashboardLayout";
import { Card } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";

const dailyData = [
  { day: "Mon", scrape: 320, search: 140, crawl: 80 },
  { day: "Tue", scrape: 450, search: 180, crawl: 120 },
  { day: "Wed", scrape: 380, search: 160, crawl: 95 },
  { day: "Thu", scrape: 520, search: 210, crawl: 150 },
  { day: "Fri", scrape: 480, search: 190, crawl: 130 },
  { day: "Sat", scrape: 210, search: 90, crawl: 45 },
  { day: "Sun", scrape: 180, search: 70, crawl: 35 },
];

const endpointData = [
  { name: "Scrape", value: 6240, color: "hsl(24, 100%, 50%)" },
  { name: "Search", value: 2890, color: "hsl(30, 100%, 60%)" },
  { name: "Crawl", value: 1840, color: "hsl(24, 60%, 40%)" },
  { name: "Map", value: 1877, color: "hsl(0, 0%, 40%)" },
];

const Usage = () => {
  return (
    <DashboardLayout>
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">Usage</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Monitor your API usage and credit consumption
          </p>
        </div>

        {/* Credit Usage */}
        <Card className="bg-card border-border p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-medium text-foreground">
              Monthly Credits
            </h2>
            <span className="text-sm text-muted-foreground">
              7,500 / 10,000
            </span>
          </div>
          <Progress value={75} className="h-2" />
          <p className="text-xs text-muted-foreground mt-2">
            Resets on March 1, 2025 · 75% used
          </p>
        </Card>

        {/* Charts */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <Card className="lg:col-span-2 bg-card border-border p-6">
            <h2 className="text-sm font-medium text-foreground mb-4">
              Daily Usage by Endpoint
            </h2>
            <div className="h-72">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={dailyData}>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="hsl(30, 10%, 90%)"
                  />
                  <XAxis
                    dataKey="day"
                    tick={{ fill: "hsl(0, 0%, 45%)", fontSize: 12 }}
                    axisLine={{ stroke: "hsl(30, 10%, 90%)" }}
                    tickLine={false}
                  />
                  <YAxis
                    tick={{ fill: "hsl(0, 0%, 45%)", fontSize: 12 }}
                    axisLine={{ stroke: "hsl(30, 10%, 90%)" }}
                    tickLine={false}
                  />
                  <Tooltip
                    contentStyle={{
                      background: "hsl(0, 0%, 100%)",
                      border: "1px solid hsl(30, 10%, 90%)",
                      borderRadius: "8px",
                      color: "hsl(0, 0%, 9%)",
                      fontSize: 13,
                    }}
                  />
                  <Bar
                    dataKey="scrape"
                    fill="hsl(24, 100%, 50%)"
                    radius={[2, 2, 0, 0]}
                  />
                  <Bar
                    dataKey="search"
                    fill="hsl(30, 100%, 60%)"
                    radius={[2, 2, 0, 0]}
                  />
                  <Bar
                    dataKey="crawl"
                    fill="hsl(24, 60%, 40%)"
                    radius={[2, 2, 0, 0]}
                  />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </Card>

          <Card className="bg-card border-border p-6">
            <h2 className="text-sm font-medium text-foreground mb-4">
              Requests by Endpoint
            </h2>
            <div className="h-52">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={endpointData}
                    cx="50%"
                    cy="50%"
                    innerRadius={50}
                    outerRadius={80}
                    dataKey="value"
                    stroke="none"
                  >
                    {endpointData.map((entry) => (
                      <Cell key={entry.name} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      background: "hsl(0, 0%, 100%)",
                      border: "1px solid hsl(30, 10%, 90%)",
                      borderRadius: "8px",
                      color: "hsl(0, 0%, 9%)",
                      fontSize: 13,
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
            <div className="space-y-2 mt-2">
              {endpointData.map((item) => (
                <div
                  key={item.name}
                  className="flex items-center justify-between text-sm"
                >
                  <div className="flex items-center gap-2">
                    <div
                      className="h-2.5 w-2.5 rounded-full"
                      style={{ background: item.color }}
                    />
                    <span className="text-muted-foreground">{item.name}</span>
                  </div>
                  <span className="font-mono text-foreground">
                    {item.value.toLocaleString()}
                  </span>
                </div>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </DashboardLayout>
  );
};

export default Usage;
