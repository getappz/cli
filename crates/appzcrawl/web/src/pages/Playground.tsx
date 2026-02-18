import {
  ArrowRight,
  Globe,
  Layers,
  Map as MapIcon,
  Search,
} from "lucide-react";
import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

const sampleResponse = {
  scrape: `{
  "success": true,
  "data": {
    "markdown": "# Example Page\\n\\nThis is the scraped content...",
    "metadata": {
      "title": "Example Domain",
      "description": "Example description",
      "language": "en",
      "sourceURL": "https://example.com",
      "statusCode": 200
    }
  }
}`,
  search: `{
  "success": true,
  "data": [
    {
      "url": "https://example.com/result1",
      "title": "Search Result 1",
      "description": "Description of result...",
      "markdown": "# Content..."
    }
  ]
}`,
  map: `{
  "success": true,
  "links": [
    "https://example.com",
    "https://example.com/about",
    "https://example.com/pricing",
    "https://example.com/docs",
    "https://example.com/blog"
  ]
}`,
  crawl: `{
  "success": true,
  "status": "completed",
  "completed": 12,
  "total": 12,
  "data": [
    {
      "markdown": "# Page 1 Content...",
      "metadata": { "sourceURL": "https://example.com" }
    }
  ]
}`,
};

const tabConfig = [
  {
    value: "scrape",
    label: "Scrape",
    icon: Globe,
    placeholder: "https://example.com",
  },
  {
    value: "search",
    label: "Search",
    icon: Search,
    placeholder: "AI web scraping tools",
  },
  {
    value: "map",
    label: "Map",
    icon: MapIcon,
    placeholder: "https://example.com",
  },
  {
    value: "crawl",
    label: "Crawl",
    icon: Layers,
    placeholder: "https://example.com",
  },
];

const Playground = () => {
  const [activeTab, setActiveTab] = useState("scrape");
  const [inputValue, setInputValue] = useState("");
  const [response, setResponse] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSubmit = () => {
    if (!inputValue.trim()) return;
    setLoading(true);
    setTimeout(() => {
      setResponse(sampleResponse[activeTab as keyof typeof sampleResponse]);
      setLoading(false);
    }, 1200);
  };

  return (
    <DashboardLayout>
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">Playground</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Test the Firecrawl API endpoints
          </p>
        </div>

        <Card className="bg-card border-border p-6">
          <Tabs
            value={activeTab}
            onValueChange={(v) => {
              setActiveTab(v);
              setResponse(null);
            }}
          >
            <TabsList className="bg-secondary border border-border mb-6">
              {tabConfig.map((tab) => (
                <TabsTrigger
                  key={tab.value}
                  value={tab.value}
                  className="data-[state=active]:bg-primary data-[state=active]:text-primary-foreground gap-2"
                >
                  <tab.icon className="h-4 w-4" />
                  {tab.label}
                </TabsTrigger>
              ))}
            </TabsList>

            {tabConfig.map((tab) => (
              <TabsContent key={tab.value} value={tab.value}>
                <div className="flex gap-3">
                  <Input
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    placeholder={tab.placeholder}
                    className="bg-secondary border-border text-foreground font-mono text-sm"
                    onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
                  />
                  <Button
                    onClick={handleSubmit}
                    disabled={loading}
                    className="gradient-orange text-primary-foreground px-6 gap-2"
                  >
                    {loading ? "Running..." : "Run"}
                    <ArrowRight className="h-4 w-4" />
                  </Button>
                </div>
              </TabsContent>
            ))}
          </Tabs>

          {/* Response */}
          {response && (
            <div className="mt-6">
              <div className="flex items-center justify-between mb-2">
                <p className="text-sm font-medium text-foreground">Response</p>
                <span className="text-xs text-success font-mono">200 OK</span>
              </div>
              <pre className="bg-muted border border-border rounded-lg p-4 overflow-auto max-h-96 text-sm font-mono text-foreground">
                {response}
              </pre>
            </div>
          )}
        </Card>
      </div>
    </DashboardLayout>
  );
};

export default Playground;
