import { HaloBar } from "./halo/HaloBar";

/**
 * The Halo appearance (`react_orb`). The value and this path are what the
 * host switch and saved settings know; the look itself lives in ./halo. The
 * host passes the appearance value; the ring has one look, so it is unused.
 */
export function ReactOrbBar(_props: { barAppearance?: string }) {
  return <HaloBar />;
}
