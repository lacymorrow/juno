import { createContext, useContext, type ReactNode } from "react";
import { useSettings } from "@/hooks/useSettings";

type SettingsContextValue = ReturnType<typeof useSettings>;

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
	const settings = useSettings();
	return (
		<SettingsContext.Provider value={settings}>
			{children}
		</SettingsContext.Provider>
	);
}

export function useSettingsContext(): SettingsContextValue {
	const ctx = useContext(SettingsContext);
	if (!ctx) {
		throw new Error("useSettingsContext must be used within a SettingsProvider");
	}
	return ctx;
}
