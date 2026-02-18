import DashboardLayout from "@/components/DashboardLayout";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";

const SettingsPage = () => {
  return (
    <DashboardLayout>
      <div className="space-y-6 max-w-2xl">
        <div>
          <h1 className="text-2xl font-semibold text-foreground">Settings</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Manage your account settings
          </p>
        </div>

        {/* Profile */}
        <Card className="bg-card border-border p-6 space-y-4">
          <h2 className="text-sm font-medium text-foreground">Profile</h2>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label className="text-muted-foreground">Name</Label>
              <Input
                defaultValue="John Doe"
                className="bg-secondary border-border text-foreground"
              />
            </div>
            <div className="space-y-2">
              <Label className="text-muted-foreground">Email</Label>
              <Input
                defaultValue="john@example.com"
                className="bg-secondary border-border text-foreground"
              />
            </div>
          </div>
          <Button className="gradient-orange text-primary-foreground">
            Save Changes
          </Button>
        </Card>

        <Separator className="bg-border" />

        {/* Notifications */}
        <Card className="bg-card border-border p-6 space-y-4">
          <h2 className="text-sm font-medium text-foreground">Notifications</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-foreground">Usage Alerts</p>
                <p className="text-xs text-muted-foreground">
                  Get notified when credits are running low
                </p>
              </div>
              <Switch defaultChecked />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-foreground">Weekly Reports</p>
                <p className="text-xs text-muted-foreground">
                  Receive weekly usage summary
                </p>
              </div>
              <Switch />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-foreground">API Error Alerts</p>
                <p className="text-xs text-muted-foreground">
                  Get notified on repeated failures
                </p>
              </div>
              <Switch defaultChecked />
            </div>
          </div>
        </Card>

        <Separator className="bg-border" />

        {/* Danger Zone */}
        <Card className="bg-card border-destructive/30 p-6 space-y-3">
          <h2 className="text-sm font-medium text-destructive">Danger Zone</h2>
          <p className="text-xs text-muted-foreground">
            Once deleted, your account cannot be recovered.
          </p>
          <Button variant="destructive" size="sm">
            Delete Account
          </Button>
        </Card>
      </div>
    </DashboardLayout>
  );
};

export default SettingsPage;
