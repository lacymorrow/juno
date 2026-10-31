# Juno for Windows

**Status:** Plan approved 2026-10-31. Nothing built.
**DRI:** The Founding Engineer owns the build. Lacy approves scope and promotes releases.
**Tracking:** LAC-6321

## The one-line problem

Computer use on Windows today takes your mouse away, and Juno can do the same work without moving it.

OpenAI's docs say Computer Use "runs on the active desktop", can't work in the background while you use the same session, and may move the pointer and take over the foreground. They suggest a Windows VM if you want to keep working ([docs](https://learn.chatgpt.com/docs/computer-use)). Claude Cowork needs Windows Pro, Enterprise or Education because its sandbox uses Hyper-V, so Windows Home is left out (secondary sources, check before quoting: [1](https://letsdatascience.com/news/anthropic-adds-computer-use-to-windows-apps-40c5c1ad), [2](https://claude-for-operators.pages.dev/platform/cowork/)). Juno targets every edition and acts through accessibility first, which is where its Mac engine already went in #749.

## The first ten seconds

Someone on a Windows 11 laptop presses the Copilot key and says "rename the screenshots in Downloads by date." The bar opens without taking focus from the document they're typing in. Explorer renames the files. Their cursor stays where it was, and the bar says "Renamed 14 files."

## Decisions

| Question | Decision |
|---|---|
| First target | Windows 11, x64 |
| Windows 10 | v1.1, the first follow-up. Lacy's own daily machine, so it stays close behind |
| Install | Per-user in v1. Optional admin install arrives in v1.1 |
| Bar looks | Pill (default) and Island are required in v1 |
| Test machine | Windows 11 VM for the spike, plus one physical Home-edition laptop before launch |

## v1

1. **Bar and chat.** Pill and Island, non-activating, with snap wells, mixed-DPI monitors and a tray icon. Island is about 1,500 lines of React with no native code, so its work is in the window frame and hit-testing it shares with the Pill. Other looks ship in v1 only if they pass QA with no extra code. Orb and Avatar are the likely ones. Notch waits, since PCs have no notch and it's still in beta on the Mac.
2. **Computer use.** `WindowsEngine` on UI Automation behind the `AccessibilityEngine` trait: element tree, find, press, window list, single-window screenshot through Windows.Graphics.Capture, and SendInput as the last resort. Add a Windows prompt fragment and a PowerShell tool behind the ported risk classifier.
3. **Voice.** Hold-to-talk on a low-level keyboard hook installed only while needed, the Copilot key (LCtrl+Win+F23) as default trigger, Whisper on Vulkan with a CPU fallback, Parakeet through ONNX Runtime, paste at cursor, and the best installed Windows voice for TTS.
4. **Ship.** Signed NSIS installer (`installMode: currentUser`), updater manifest for windows-x86_64, one-screen onboarding and a download page. Windows has no Accessibility or Screen Recording permission to ask for, only the microphone privacy toggle.

## v1.1

- **Windows 10.** Build v1 so this is a small step: check for newer APIs at runtime and never assume Windows 11. Known differences to confirm: borderless capture needs build 20348 or later, so Windows 10 22H2 keeps the capture border. Few Windows 10 machines have a Copilot key, so it needs another default trigger. Natural voices may be missing.
- **Admin windows, through an optional admin install.** Windows blocks input to elevated windows unless the sender has `uiAccess`, which needs a trusted signature and a location under Program Files ([Microsoft](https://learn.microsoft.com/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-only-elevate-uiaccess-applications-that-are-installed-in-secure-locations)). The likely shape is a small signed helper in Program Files that the per-user app talks to, so nobody reinstalls Juno and settings stay put. UAC prompts stay out of reach either way.
- **Now Playing card,** on the Windows media session API (`GlobalSystemMediaTransportControlsSessionManager`), which covers any player that reports to Windows.

## Later

ARM64, the rest of the bar looks, share sheet, settings vibrancy, MCP registry registration, borderless capture consent, and an agent desktop on Microsoft's MXC `isolation_session` backend (own account, desktop and input). That backend is experimental and needs an Insider build ([wiki](https://github.com/microsoft/mxc/wiki)), so it can't ship yet.

Safari tools don't come to Windows. Chrome and Edge already work over CDP.

## States to design

- **Admin window in v1.** The agent says "That window is running as administrator, so I can't use it." It never fails silently.
- **First run is a SmartScreen warning.** Azure Artifact Signing doesn't give instant reputation ([Microsoft](https://learn.microsoft.com/windows/msix/package/sign-msix-package-guide)), and some signers reported regressions in April 2026 ([Q&A](https://learn.microsoft.com/en-us/answers/questions/5855708/trusted-signing-regression-in-smartscreen-reputati)). The download page shows the warning and the click past it.
- **Antivirus.** A keyboard hook, synthetic input and screen capture together look like a keylogger to heuristics. Sign every binary and submit each release to Microsoft's false-positive portal before announcing it.
- **Capture border.** Windows draws a yellow border during capture unless the person grants borderless access. v1 keeps it and explains it once.
- **Chromium and Electron apps.** Their UI Automation tree is built lazily, hidden while the window isn't visible, and can take over 300 ms to appear. Send `WM_GETOBJECT` to wake it and wait on the tree, not a fixed sleep ([notes](https://github.com/Descolada/UIAutomation/wiki/08.-Common-pitfalls;-tips-and-tricks)).
- **Juno's overlays in agent screenshots.** Exclude them with `contentProtected`, except where someone wants the bar visible to a screen share (#836).

## Order of work

0. **Spike.** Prove the engine before anything else (below).
1. Bar and chat, with no computer use.
2. Computer use, with one task list run on both Mac and Windows.
3. Voice, which needs whisper-rs moved from 0.11 to 0.16 for its `vulkan` feature.
4. Signing, installer, onboarding, site copy.
5. v1.1, starting with Windows 10.

## The spike

The smallest thing that can prove this is `juno-cua`, the headless CLI, because it depends on the automation crate and has no Tauri app to port. The full app stays out of the spike.

1. Move the Mac-only dependencies in `src-tauri/mcp-server-os-level/Cargo.toml` behind `cfg(target_os = "macos")`, along with the code that uses them.
2. Replace `platforms/windows.rs`. It doesn't compile today because it imports a `crate::operator` module that no longer exists, and `create_engine` returns an error before reaching it anyway.
3. Add a separate Windows workflow that builds `juno-cua` on `windows-latest`, outside the required checks until it's stable. The Mac jobs must stay green and unchanged.
4. Implement what the test needs. The trait has 38 required methods out of 55, and the rest of them return `UnsupportedPlatform`.

**The test.** On Windows 11, with Notepad in front of Calculator, `juno-cua` presses 7 × 6 = through UI Automation Invoke and reads back 42. The cursor position and foreground window are identical before and after. Calculator is a UWP app hosted in `ApplicationFrameHost`, so the window's process isn't the app's process.

**Also measure, with no pass or fail:** tree read time for File Explorer and one Electron app, and whether a rename works in a covered Explorer window.

If the test can't pass, stop and rewrite this plan, because the claim at the top of this file depends on it.

**Evidence:** a recording plus raw before and after output, in `docs/changelog/media/<PR>/`.

## Removed from this plan

Linux, a separate Windows UI, depending on `terminator-rs` (last published December 2025, 226 downloads in 90 days), and Windows-specific marketing before the spike passes.

## Open questions

- Does typing work in a WebView2 inside a `WS_EX_NOACTIVATE` window? tao sets that style for `focusable: false`, and the bar will probably activate itself when the input opens, as it does on the Mac. Answer in step 1.
- How fast is Whisper on Vulkan on a mid-range laptop? Answer in step 3.
- Can the `uiAccess` helper be signed and installed cleanly alongside a per-user app? Answer before v1.1.
