declare module "@vitejs/plugin-react" {
  import { Plugin } from "vite";
  const plugin: () => Plugin;
  export default plugin;
}