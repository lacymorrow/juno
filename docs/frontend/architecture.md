# Frontend Architecture

The frontend is a TypeScript React application initialized via Vite.

## Structure

```
src/
├── components/     # Reusable UI components
├── contexts/       # Global state providers (Voice, etc.)
├── hooks/          # Custom logic hooks (Events, AppState)
├── types/          # TypeScript interfaces
└── lib/            # Utilities (audio, formatting)
```

## Entry Point (`main.tsx` & `App.tsx`)
- **`main.tsx`**: Mounts the app and wraps it in strictly necessary providers (Theme, TooltipProvider).
- **`App.tsx`**: The orchestration layer.
  - **Initialization**: Calls `init_app` on mount.
  - **View Routing**: Switches between `<FloatingBar />`, `<PermissionsManager />`, and `<Onboarding />` based on backend state.
  - **Global Listeners**: Mounts `useBackendEvents` and `useShortcutEvents` to start listening immediately.

## Key Libraries
- **UI**: `radix-ui` primitives, `lucide-react` icons, `framer-motion` for animations.
- **State**: React Context + Custom Hooks. We avoid Redux/Recoil to keep the "floating window" lightweight.
- **Build**: Vite with `@vitejs/plugin-react` and `vite-tsconfig-paths`.
