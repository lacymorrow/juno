declare module "@tailwindcss/vite" {
  import { Plugin } from "vite";
  const plugin: () => Plugin;
  export default plugin;
}