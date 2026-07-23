import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useEventListener } from "@/hooks/useEventListener";

interface ScheduledAutomation {
  id: string;
  name: string;
  query: string;
  cron: string;
  natural_language?: string | null;
  enabled: boolean;
  notify: boolean;
  created_at: number;
  last_run_at?: number | null;
  next_run_at?: number | null;
  last_result?: string | null;
}

interface AutomationForm {
  name: string;
  query: string;
  cron: string;
  naturalLanguage: string;
  notify: boolean;
}

const emptyForm: AutomationForm = {
  name: "",
  query: "",
  cron: "",
  naturalLanguage: "",
  notify: true,
};

const formatTime = (unixSeconds?: number | null) =>
  unixSeconds ? new Date(unixSeconds * 1000).toLocaleString() : "—";

export default function AutomationsSettings() {
  const [automations, setAutomations] = useState<ScheduledAutomation[]>([]);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form, setForm] = useState<AutomationForm>(emptyForm);
  const [previewTimes, setPreviewTimes] = useState<number[]>([]);
  const [cronError, setCronError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const loadAutomations = useCallback(async () => {
    try {
      const list = await invoke<ScheduledAutomation[]>("list_scheduled_tasks");
      setAutomations(list);
    } catch (error) {
      console.error("Failed to load scheduled automations:", error);
      toast.error("Failed to load automations");
    }
  }, []);

  useEffect(() => {
    loadAutomations();
  }, [loadAutomations]);

  // The backend emits this whenever the list changes (including agent-created schedules)
  useEventListener("scheduled-automations-changed", () => {
    loadAutomations();
  });

  useEventListener<{ name: string; success: boolean; error?: string }>(
    "scheduled-automation-fired",
    (payload) => {
      if (payload.success) {
        toast.info(`Automation "${payload.name}" is running`);
      } else {
        toast.error(`Automation "${payload.name}" failed: ${payload.error ?? "unknown error"}`);
      }
    }
  );

  const previewCron = useCallback(async (cron: string) => {
    if (!cron.trim()) {
      setPreviewTimes([]);
      setCronError(null);
      return;
    }
    try {
      const times = await invoke<number[]>("preview_cron_schedule", { cron });
      setPreviewTimes(times);
      setCronError(null);
    } catch (error) {
      setPreviewTimes([]);
      setCronError(String(error));
    }
  }, []);

  // Debounce keystrokes in the cron field so each character doesn't fire an IPC call
  const previewDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const schedulePreview = useCallback(
    (cron: string) => {
      if (previewDebounceRef.current) {
        clearTimeout(previewDebounceRef.current);
      }
      previewDebounceRef.current = setTimeout(() => previewCron(cron), 300);
    },
    [previewCron]
  );
  useEffect(
    () => () => {
      if (previewDebounceRef.current) {
        clearTimeout(previewDebounceRef.current);
      }
    },
    []
  );

  const openCreateDialog = () => {
    setEditingId(null);
    setForm(emptyForm);
    setPreviewTimes([]);
    setCronError(null);
    setDialogOpen(true);
  };

  const openEditDialog = (automation: ScheduledAutomation) => {
    setEditingId(automation.id);
    setForm({
      name: automation.name,
      query: automation.query,
      cron: automation.cron,
      naturalLanguage: automation.natural_language ?? "",
      notify: automation.notify,
    });
    setCronError(null);
    previewCron(automation.cron);
    setDialogOpen(true);
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const payload = {
        name: form.name,
        query: form.query,
        cron: form.cron,
        naturalLanguage: form.naturalLanguage || null,
        notify: form.notify,
      };
      if (editingId) {
        await invoke("update_scheduled_task", { id: editingId, ...payload });
        toast.success("Automation updated");
      } else {
        await invoke("create_scheduled_task", payload);
        toast.success("Automation created");
      }
      setDialogOpen(false);
      loadAutomations();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (automation: ScheduledAutomation, enabled: boolean) => {
    try {
      await invoke("update_scheduled_task", { id: automation.id, enabled });
      loadAutomations();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const handleDelete = async (automation: ScheduledAutomation) => {
    try {
      await invoke("delete_scheduled_task", { id: automation.id });
      toast.success(`Deleted "${automation.name}"`);
      loadAutomations();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const handleRunNow = async (automation: ScheduledAutomation) => {
    try {
      await invoke("run_scheduled_task_now", { id: automation.id });
      toast.success(`Started "${automation.name}"`);
      loadAutomations();
    } catch (error) {
      toast.error(String(error));
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Scheduled Automations</CardTitle>
              <CardDescription>
                Recurring agent tasks that run on a schedule. You can also ask the
                agent directly — e.g. "check my emails every morning".
              </CardDescription>
            </div>
            <Button onClick={openCreateDialog}>New automation</Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {automations.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No automations yet. Create one here, or ask the agent to schedule
              something for you.
            </p>
          ) : (
            automations.map((automation) => (
              <div
                key={automation.id}
                className="flex items-start justify-between gap-4 rounded-lg border p-4"
              >
                <div className="min-w-0 space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{automation.name}</span>
                    {!automation.enabled && <Badge variant="outline">Paused</Badge>}
                    {automation.last_result?.startsWith("error") && (
                      <Badge variant="destructive">Last run failed</Badge>
                    )}
                  </div>
                  <p className="truncate text-sm text-muted-foreground">
                    {automation.query}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {automation.natural_language || automation.cron}
                    {" · next run "}
                    {automation.enabled ? formatTime(automation.next_run_at) : "—"}
                    {automation.last_run_at
                      ? ` · last run ${formatTime(automation.last_run_at)}`
                      : ""}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleRunNow(automation)}
                  >
                    Run now
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => openEditDialog(automation)}
                  >
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(automation)}
                  >
                    Delete
                  </Button>
                  <Switch
                    checked={automation.enabled}
                    onCheckedChange={(checked) => handleToggle(automation, checked)}
                  />
                </div>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {editingId ? "Edit automation" : "New automation"}
            </DialogTitle>
            <DialogDescription>
              The agent runs the query below on the schedule you set. Times use
              your local timezone.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="automation-name">Name</Label>
              <Input
                id="automation-name"
                value={form.name}
                placeholder="Morning email check"
                onChange={(e) => setForm({ ...form, name: e.target.value })}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="automation-query">Agent query</Label>
              <Textarea
                id="automation-query"
                value={form.query}
                placeholder="Check my emails and summarize anything important"
                onChange={(e) => setForm({ ...form, query: e.target.value })}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="automation-cron">Schedule (cron)</Label>
              <Input
                id="automation-cron"
                value={form.cron}
                placeholder="0 9 * * MON (every Monday at 9am)"
                onChange={(e) => {
                  setForm({ ...form, cron: e.target.value });
                  schedulePreview(e.target.value);
                }}
              />
              {cronError ? (
                <p className="text-xs text-destructive">{cronError}</p>
              ) : previewTimes.length > 0 ? (
                <p className="text-xs text-muted-foreground">
                  Next runs: {previewTimes.map((t) => formatTime(t)).join(", ")}
                </p>
              ) : null}
            </div>
            <div className="space-y-2">
              <Label htmlFor="automation-nl">Description (optional)</Label>
              <Input
                id="automation-nl"
                value={form.naturalLanguage}
                placeholder="every Monday at 9am"
                onChange={(e) =>
                  setForm({ ...form, naturalLanguage: e.target.value })
                }
              />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="automation-notify">Notify when it runs</Label>
              <Switch
                id="automation-notify"
                checked={form.notify}
                onCheckedChange={(checked) => setForm({ ...form, notify: checked })}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleSave}
              disabled={
                saving ||
                !form.name.trim() ||
                !form.query.trim() ||
                !form.cron.trim() ||
                !!cronError
              }
            >
              {editingId ? "Save changes" : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
