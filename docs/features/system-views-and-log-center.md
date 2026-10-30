# System views and log center

Recent, Applications and Trash open inside Fiex as virtual directories on Windows, macOS and Linux. Recent remains read-only. Trash exposes dedicated restore and permanent-delete actions rather than general file transfer.

| Behavior | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Application imports | Valid executable `.lnk` shortcuts and `.exe` files | `.app` bundles, Finder application aliases and executable files | Valid `.desktop` launchers, AppImage files, executable files and executable symlinks |
| Imported storage | `~/.fiex/applications` | `~/.fiex/applications`; bundles are copied, originals retained | `~/.fiex/applications`; executable symlinks become `.desktop` launchers and retain their originals |
| User-created application folders | Private application store | Private application store | Private application store |
| Open application | Launch application | Launch application, including imported bundles | Launch desktop entry or executable |
| System application actions | Launch and organize; no delete, uninstall or export | Launch and organize; no delete, uninstall or export; native `/Applications`, `/System/Applications` and `/System/Library/CoreServices` are protected from mutation | Launch and organize; no delete, uninstall or export |
| User-imported items | Manage within the private store | Manage within the private store | Manage within the private store |
| Trash | Virtual directory backed by Windows Recycle Bin identities | Virtual directory backed by macOS Trash | Virtual directory backed by Linux Trash |

Import authorization and transfer preflight run before writes. macOS bundle import uses `ditto` to preserve bundle content and links, rechecks source metadata and destination policy before committing, and supports cancellation. Existing bundle replacement requires rename or keep-both. Bundles and executable symlinks must be imported separately from other file types in a mixed selection.

On macOS, denied Trash access goes through the normal unified `reportError` path. The message directs users to **System Settings → Privacy & Security → Full Disk Access**, enable Fiex, quit and reopen it. Access is checked before painting cached Trash metadata.

Provider settings use feature tabs, provider cards, enable switches, drag ordering, keyboard-accessible move buttons and reset controls. Locked fallback providers remain enabled.

The Status Center includes the log entry. One log center switches between Core and plugin sources, with search, severity filtering, pause, copy, export, clear and expandable diagnostic details. Existing SDK diagnostics and bounded incremental log APIs remain the data source.

Final tests, builds and runtime acceptance were skipped at the user's request. Cross-platform native behavior and visual acceptance remain pending.
