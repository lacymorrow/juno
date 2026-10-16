/**
 * Form: ask the person for a few things and take the answers as their next turn.
 *
 * A shadcn Card around a real `<form>`. The agent puts `Field`s in it, each
 * holding one shadcn control (Input, Textarea, Switch, Select, Checkbox,
 * Slider), and writes the label on the one primary button. Submit reads the
 * controls' values and sends them as the person's next message through the
 * same `dispatch_query` command QueryButton uses. This file holds no logic
 * beyond that formatting: the agent decides what the answers mean.
 *
 * The controls are only drawn inside a Form (see `formOnly`). A control that
 * sits alone in an answer would be bound to nothing, which is the dead
 * control defect, so outside a Form it renders nothing at all.
 */

import * as React from "react";
import { dispatchQuery } from "@/components/ui/agent-actions";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

interface FormContextValue {
  /** The element the select list is portalled into, so it keeps the tokens. */
  container: HTMLElement | null;
}

const FormContext = React.createContext<FormContextValue | null>(null);

/** A control that draws only inside a Form. */
function formOnly<P extends object>(Control: React.ComponentType<P>, name: string): React.ComponentType<P> {
  const Guarded = (props: P) => (React.useContext(FormContext) ? <Control {...props} /> : null);
  Guarded.displayName = `Form(${name})`;
  return Guarded;
}

export const FormInput = formOnly(Input, "Input");
export const FormTextarea = formOnly(Textarea, "Textarea");
export const FormCheckbox = formOnly(Checkbox, "Checkbox");
export const FormSwitch = formOnly(Switch, "Switch");
export const FormSlider = formOnly(Slider, "Slider");
export const FormSelect = formOnly(Select, "Select");
export const FormSelectTrigger = formOnly(SelectTrigger, "SelectTrigger");
export const FormSelectValue = formOnly(SelectValue, "SelectValue");
export const FormSelectItem = formOnly(SelectItem, "SelectItem");

export function FormSelectContent(props: React.ComponentProps<typeof SelectContent>) {
  const ctx = React.useContext(FormContext);
  return ctx ? <SelectContent {...props} container={ctx.container} /> : null;
}

/** Toggles read as "control, then label" on one line. The flag survives the renderer's class-filter wrapper. */
export const INLINE_CONTROL = "inlineControl";
(FormCheckbox as unknown as Record<string, boolean>)[INLINE_CONTROL] = true;
(FormSwitch as unknown as Record<string, boolean>)[INLINE_CONTROL] = true;

// ============================================================
// Field: a label, one control, an optional hint
// ============================================================

export interface FieldProps {
  label?: string;
  /** A short line under the control. */
  hint?: string;
  children?: React.ReactNode;
  className?: string;
}

export function Field({ label, hint, children, className }: FieldProps) {
  const id = React.useId();
  const inForm = React.useContext(FormContext) !== null;
  if (!inForm) return null;
  const only = React.Children.count(children) === 1 ? React.Children.toArray(children)[0] : null;
  const control =
    React.isValidElement<{ id?: string }>(only) && !only.props.id ? React.cloneElement(only, { id }) : children;
  const inline =
    React.isValidElement(only) && !!(only.type as unknown as Record<string, boolean | undefined>)[INLINE_CONTROL];

  return (
    <div data-slot="form-field" className={cn("space-y-1.5", inline && "space-y-0", className)}>
      {inline ? (
        <div className="flex items-center gap-2">
          {control}
          {label && (
            <Label htmlFor={id} className="text-[13px] font-normal">
              {label}
            </Label>
          )}
        </div>
      ) : (
        <>
          {label && (
            <Label htmlFor={id} className="text-[11px] font-medium text-muted-foreground">
              {label}
            </Label>
          )}
          {control}
        </>
      )}
      {hint && <p className="text-[11px] text-muted-foreground">{hint}</p>}
    </div>
  );
}

// ============================================================
// Form
// ============================================================

/** "due_date" and "dueDate" both read "Due date". */
function labelFor(name: string): string {
  const spaced = name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .trim()
    .toLowerCase();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

type NamedControl = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

/** The form's values as "Label: value" lines, in document order. Blank answers are left out. */
export function readFormLines(form: HTMLFormElement): string[] {
  const order: string[] = [];
  const values = new Map<string, string[]>();
  for (const el of Array.from(form.elements) as NamedControl[]) {
    const name = el.getAttribute("name");
    if (!name || el.tagName === "BUTTON" || (el as HTMLInputElement).type === "submit") continue;
    const value =
      (el as HTMLInputElement).type === "checkbox" ? ((el as HTMLInputElement).checked ? "Yes" : "No") : el.value.trim();
    if (!value) continue;
    if (!values.has(name)) {
      values.set(name, []);
      order.push(name);
    }
    values.get(name)?.push(value);
  }
  return order.map((name) => `${labelFor(name)}: ${values.get(name)?.join(", ")}`);
}

export interface FormProps {
  title?: string;
  /** The one primary button's label, as a verb: "Create reminder". */
  submit?: string;
  /** The first line of the message sent back, in the person's voice. */
  intro?: string;
  children?: React.ReactNode;
  className?: string;
}

export function Form({ title, submit = "Send", intro, children, className }: FormProps) {
  const [container, setContainer] = React.useState<HTMLFormElement | null>(null);
  const [status, setStatus] = React.useState<"idle" | "sending" | "sent" | "error">("idle");

  const onSubmit = React.useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (status === "sending" || status === "sent") return;
      const lines = readFormLines(event.currentTarget);
      const lead = intro || (title ? `${title}:` : "Here are my answers:");
      setStatus("sending");
      try {
        await dispatchQuery([lead, ...lines].join("\n"));
        // The message appearing in the conversation is the feedback.
        setStatus("sent");
      } catch (err) {
        console.error("Form: failed to send the answers:", err);
        setStatus("error");
      }
    },
    [intro, status, title],
  );

  if (React.Children.count(children) === 0) return null;
  const locked = status === "sending" || status === "sent";

  return (
    <FormContext.Provider value={{ container }}>
      <form ref={setContainer} onSubmit={onSubmit} data-slot="agent-form" data-state={status} className={className}>
        <Card>
          {title && (
            <CardHeader>
              <CardTitle className="min-w-0 truncate">{title}</CardTitle>
            </CardHeader>
          )}
          <CardContent className={cn("pt-1", !title && "pt-3")}>
            <fieldset disabled={locked} className="min-w-0 space-y-3 border-0 p-0">
              {children}
            </fieldset>
          </CardContent>
          <CardFooter className="flex-wrap">
            {status === "error" && (
              <span role="status" className="mr-auto text-[11px] text-muted-foreground">
                Could not send that. Try again.
              </span>
            )}
            <Button type="submit" size="card" disabled={locked}>
              {status === "sent" ? "Sent" : submit}
            </Button>
          </CardFooter>
        </Card>
      </form>
    </FormContext.Provider>
  );
}
