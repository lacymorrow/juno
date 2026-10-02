import { AppearancePicker } from "@/components/settings/AppearancePicker";
import { useBarAppearance } from "@/components/settings/useBarAppearance";

/**
 * Onboarding's "Pick how Juno looks" body. It is the Settings picker, not a
 * copy: the same component, saved through the same hook. The saved look (the
 * default on a fresh install) is already selected, so Continue with no choice
 * is a complete answer.
 */
export function AppearanceStep() {
  const { value, saving, change } = useBarAppearance();
  return (
    <div className="pt-4 text-left">
      <AppearancePicker value={value} onChange={change} disabled={saving} />
    </div>
  );
}
