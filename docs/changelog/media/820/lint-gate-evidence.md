# PR #820 — the lint gate, demonstrated

Raw output, captured 2026-11-03 on the rebased branch (main + 60 commits).

## 1. Clean on the whole tree

```
$ bun run lint
$ eslint . --max-warnings 0
(no output, exit 0)

# independent count, same run, via the json formatter:
files linted: 418
errors: 0 warnings: 0
```

The file count is the tripwire. A config that silently matched zero files
would also exit 0, so "no findings" is only meaningful next to "418 files".

## 2. The gate actually fires

A probe file planting one violation per enforced rule:

```
see below
  line  2: no-restricted-imports            (@tauri-apps/plugin-websocket)
  line  7: react-hooks/set-state-in-render
  line  8: no-restricted-globals            (localStorage)
  line  9: no-restricted-globals            (sessionStorage)
  line 10: no-restricted-globals            (WebSocket)
  line 11: no-restricted-globals            (AudioContext)
  line 12: no-restricted-syntax             (navigator.mediaDevices.getUserMedia)
  line 15: no-restricted-syntax             (data-tauri-drag-region)
TOTAL: 8 findings, eslint exit 1
```

8 of 8 enforced rules fire. The probe also planted a ref-read-during-render,
which is NOT caught: `react-hooks/refs` is one of the 8 deferred rules
(64 findings, listed in eslint.config.js, tracked in LAC-4500). That miss is
the documented ratchet gap, not a hole in the gate.

## 3. It does not break anyone else

Every open PR that touches frontend code, linted with this config at the
merge result CI would actually evaluate (actions/checkout on a pull_request
event checks out the merge commit, not the head):

| PR | frontend files | result |
|---|---|---|
| 938 windows computer use | 27 | pass |
| 935 windows app build | 23 | pass |
| 846 island resting frame | 7 | pass |
| 740 Composio phase 1 | 8 | pass |
| 866 island markdown | 5 | pass |
| 870 editable trigger sentences | 4 | pass |
| 864 frontend logs | 4 | pass |
| 835 display hop | 4 | pass |
| 815 permissions card | 4 | pass |
| 929 model picker | 3 | pass |
| 869 audio device list | 3 | pass |
| 872 dead useDockSlot | 2 | pass |
| 843 Haiku 5.5 | 1 | pass |

Tested head-only first, where 938/935/870 each showed 2 errors on the two
pre-existing `data-tauri-drag-region` attributes in ModularSettingsWindow.tsx.
Those are the violations this PR suppresses under LAC-4502, so the branches
only fail without this PR merged in. Against the merge result, which is what
CI lints, all three are clean.
