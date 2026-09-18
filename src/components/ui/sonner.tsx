import { useTheme } from "next-themes"
import { Toaster as Sonner, ToasterProps } from "sonner"

/**
 * The app's one toast surface.
 *
 * The defaults live here rather than at the mount site so every window gets the
 * same notice. They are deliberately plain: a flat popover card on the system
 * surface, no rich colors, no glow. `expand` stays off so a burst of notices
 * collapses into sonner's stack instead of printing a column of identical
 * full-width alert cards.
 */
const Toaster = ({ ...props }: ToasterProps) => {
  const { theme = "system" } = useTheme()

  return (
    <Sonner
      theme={theme as ToasterProps["theme"]}
      className="toaster group"
      position="top-center"
      expand={false}
      richColors={false}
      closeButton={true}
      duration={3000}
      // Three is enough to show something is piling up without covering the
      // conversation in the floating bar, which is only 419px wide.
      visibleToasts={3}
      gap={8}
      // Sonner styles itself from these variables, so the card follows the app's
      // popover tokens in both light and dark rather than carrying its own palette.
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
        } as React.CSSProperties
      }
      {...props}
    />
  )
}

export { Toaster }
