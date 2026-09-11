# Marketing media

Screenshots, tray renders, and the social-card image (`og.png`). These files
were moved out of `public/` on purpose: everything in `public/` is copied into
`dist/` and ships inside the app bundle, and none of these are referenced by
the app at runtime (~14 MB of dead weight).

Still in `public/` because the app uses them:

- `public/juno.png` — onboarding welcome logo and toggle demo
  (`src/components/onboarding/Onboarding.tsx`)
- `public/juno5.png` — favicon (`index.html`) and the README logo
  (referenced by raw.githubusercontent.com URL)

`og.png` is referenced from `index.html`'s `og:image` / `twitter:image` meta
tags by absolute GitHub raw URL pointing at this directory. If you move or
rename it, update those tags.

Before adding a new image, ask where it is consumed: app at runtime →
`public/`; docs, README, or social cards → here.
