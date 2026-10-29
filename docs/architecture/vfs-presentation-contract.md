# Builtin VFS, filesystem and presentation contract

This is the current behavior contract for Recent, Applications, Trash, plugin VFS/connection mounts and local/USB filesystems. Core owns routing, resource identity, scopes and result validation. An adapter implements storage operations; a resolver selects and combines presentation contributions. Official plugins have exactly the same permissions and interfaces as third-party plugins.

## The three builtin VFS adapters

| VFS                         | Enumeration and metadata                                                  | Read/open                                                                                  | Writes                                                                                                             | Backing store                                                                                    |
| --------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ |
| Recent                      | List the recorded original resources; stat comes from their owning FS/VFS | Open/preview/copy supported when the original resource supports reading                    | No upload, create, rename, cut, duplicate or remove through the Recent view                                        | Original resource; never a `recent://` directory on disk                                         |
| Applications, root          | Merge platform application catalog and user store                         | Discovered applications launch using platform identity; they are not readable file streams | Accept file/folder copy/import and create file/folder into the user store; never modify the platform catalog       | `~/.fiex/applications`                                                                           |
| Applications, user entry    | Stat/list and nested directories resolve relative to user store           | Read/preview/copy files; launch stored shortcuts/bundles with the OS                       | Create/write/rename/remove subject to actual stored entry restrictions                                             | Only inside `~/.fiex/applications`; relative URI paths, traversal and escaping symlinks rejected |
| Trash / Windows Recycle Bin | Enumerate deleted entries and metadata                                    | No normal open or navigation; restore is the way to recover content                        | Restore to original location (or explicit fallback), permanent delete, empty; no ordinary uploads/new files/rename | Platform Trash / Shell Recycle Bin                                                               |

Applications writes are connected to the desktop transfer/create/rename commands. Transfer destinations are resolved to the private user store before the existing conflict, progress and cancellation engine runs. Successful result paths are projected back to application URIs. System applications are launchable identities and have no store mutation operations. A mixed system/user selection offers only actions common to every selected item.

These are in-process Core adapters. Their presentation events use the same EventRouter, opaque resource references, matching, validation and expiring single-use menu sessions as plugin adapters. Core does not launch its own builtins through plugin IPC. Plugin providers continue to implement the canonical `vfs.*` stream/discovery contract; Core adapters use their local backing-store/OS implementations behind the same operation policy. `restore`/`empty` are Trash-specific actions, not invented generic VFS writes.

## Capability conformance

| Builtin VFS             | list/stat | Content read                   | File write/import    | mkdir/rename                          | remove               | Extra actions          |
| ----------------------- | --------- | ------------------------------ | -------------------- | ------------------------------------- | -------------------- | ---------------------- |
| Recent                  | Yes       | Original resource, if readable | No                   | No                                    | No                   | Open/preview/copy      |
| Applications root       | Yes       | Catalog has no file stream     | Yes, user store only | mkdir in user store                   | Not the root/catalog | Launch discovered app  |
| Applications user nodes | Yes       | Stored files                   | Stored files         | User store only; mkdir on directories | User entries only    | Launch shortcut/bundle |
| Trash                   | Yes       | No normal open/read contract   | No                   | No                                    | Permanent deletion   | Restore/empty          |

`VirtualResolver.operations(path)` is the explicit Core adapter ceiling. `Entry.allowedOps` and actual resource flags narrow it. Unsupported methods are absent; a VFS is not required to implement every possible operation. Core-backed storage routes run in process through the desktop command/OS adapter; the plugin `vfs.*` stream RPC is the out-of-process implementation of the storage contract, not a second IPC process for builtins.

## Readonly, operations and provenance

- `Entry.readonly` describes an explicit resource restriction. The absence of `write`, `mkdir`, `rename` or `remove` must never synthesize this attribute or its badge.
- A view may narrow available operations without changing the original entry's `readonly`. Recent does this: `Listing.readOnly=true`, and event scopes reject write operations; a writable original file receives no readonly badge just because it is shown in Recent.
- Plugin operations are the provider declaration intersected with per-node restrictions. Discovery and command dispatch remain authoritative and can deny a stale menu snapshot. An offline/unreachable provider must not gain permissions from cached local bytes.
- `write` changes or creates file content; `mkdir` creates a child directory; `rename` and `remove` are independent. A provider permitting deletion but not upload is not necessarily readonly.
- Windows directory `FILE_ATTRIBUTE_READONLY` is commonly shell customization metadata, not proof of write denial. It does not produce a readonly badge or disable folder creation. The OS still checks ACLs when an operation runs. ACL errors do not turn every folder into a readonly resource.
- Local Unix content mode bits do not by themselves deny removal/renaming: those also depend on parent-directory permissions. Actual operations remain subject to the OS, ACLs and current access; no UI badge promises effective ACL access.
- The **System / 系统** badge means a platform-enumerated item in the Applications view. It never marks arbitrary OS files, `.app` folders seen in an ordinary filesystem, Recent entries, Trash items or plugin files. It does not mean readonly.
- User Applications entries get their stored shortcut/bundle icon and no System badge. Explicit readonly entries may still receive the readonly badge independently.

## Presentation event envelope

`contextMenu.resolve/execute` and `decorations.resolve` use the existing resource-ref envelope and approved manifest contributions. `targets` is exactly the event's selected/visible resource list. The directory is not prepended as another selected file (which previously broke extension matching and selection counts).

Additive raw event fields are available to providers through their request parameters:

```json
{
  "location": { "kind": "vfs", "providerId": "core:applications", "selectionCount": 1 },
  "targets": [
    {
      "ref": { "mountId": "core:applications", "nodeId": "opaque" },
      "metadata": {
        "name": "My shortcut.lnk",
        "kind": "file",
        "readonly": false,
        "operations": {
          "read": true,
          "write": true,
          "mkdir": false,
          "rename": true,
          "remove": true
        }
      }
    }
  ]
}
```

`location.kind` is `fs` or `vfs`; Core builtin IDs are `core:recent`, `core:applications`, `core:trash`. Plugin mount locations carry their owning plugin ID. Metadata and operation hints are omitted without approved metadata permission. These are additive raw fields, not new required SDK struct members. They are hints for presentation, never authorization tokens. ResourceRefs and Core-owned scopes are authoritative; a provider never receives an ambient OS path from a virtual URI.

Plugins opt into empty-canvas menus using `match.selection: "zero"`. Empty selection stays empty. An extension must declare `contextMenus` / `decorations` and pass normal matching and permission checks even when it also owns the current VFS. Core's builtin VFS no longer bypasses extension aggregation with an early return.

Decoration responses may identify an entry with its `ref`. Core maps that ref only within the request's visible resources. Legacy `name` is accepted only when unique; duplicate display names never pick an arbitrary file. All badges/styles/icons still pass sanitization. Native status badges cannot be forged by a plugin response.

## Scenario matrix

| VFS / FS and content                                | Context menu                                                                                           | Decoration                                                                        |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| Local FS, ordinary writable file                    | Open, preview/info, copy/cut, duplicate, rename, platform Trash as supported                           | Tags; no System/readonly badge                                                    |
| Local FS, explicit readonly file                    | Read/open/preview/copy; content writes absent; OS/parent permissions decide rename/remove              | Readonly; never System                                                            |
| Windows FS, directory with READONLY shell attribute | Navigation and normal creation/mutation remain available; OS ACL checks actual call                    | No fabricated readonly badge                                                      |
| Local FS, `.app` / executable outside Applications  | Ordinary filesystem operations under real permissions                                                  | Never System just from extension                                                  |
| Recent, ordinary writable original                  | Open, preview/info and copy; no create/rename/cut/duplicate/delete through this view                   | Original tags/status; no readonly badge from the view                             |
| Recent, explicitly readonly original                | Same nonmutating operations, subject to source readability                                             | Readonly from source only                                                         |
| Recent, blank canvas                                | No builtin creation; opted-in plugin actions are subject to readonly scope                             | No row badges                                                                     |
| Applications, root blank canvas                     | New file/folder writes into `.fiex/applications`; pasted/copied entries go there too                   | No row badges                                                                     |
| Applications, discovered system application         | Launch only; no uninstall, rename or delete through this catalog                                       | System and native logo; no fabricated readonly badge                              |
| Applications, stored user file/shortcut             | Open, preview/info, copy/cut, duplicate, rename and permanent delete according to operations           | Stored icon; no System; actual readonly flag only                                 |
| Applications, stored user directory                 | Navigate, create children, copy/rename/permanent delete under store restrictions                       | No System; actual readonly flag only                                              |
| Applications, mixed discovered/user selection       | Intersection: launch only                                                                              | Each row keeps its own provenance/flags                                           |
| Trash / Recycle Bin, deleted file or folder         | Restore / permanent delete; no open/new/rename/ordinary write                                          | No System; no readonly inference from Trash policy                                |
| Plugin standalone VFS, readable nonwritable node    | Read/open/preview/copy and declared plugin actions; independent remove/rename may still be present     | No readonly badge unless provider explicitly sets it                              |
| Plugin connection VFS, writable node                | Capability-gated create/rename/write/remove; deletion is permanent without provider Trash              | Provider decoration plus actual readonly/status                                   |
| Connection cache, cloud/pinned/pending/conflict     | Cache/pin/release/conflict actions from connection resolver; never based on a local placeholder's mode | Cache/upload/pin/conflict badges; uploaded only while cache or pin exists         |
| USB NTFS/ext4 adapter                               | Driver-supported read/create/rename/remove; no local terminal/tags/platform Trash                      | Driver entry flags; no System                                                     |
| USB APFS/HFS/Btrfs readonly adapter                 | Read/open/preview/copy; mutating groups absent                                                         | Driver readonly badge                                                             |
| Mixed node selection                                | Intersection of every node's operations; rechecked at execution                                        | Per-entry decoration, not a selection-wide flag                                   |
| Search result                                       | Actions resolve using each source resource; builtin decorations use actual entry flags                 | No parent directory inserted into visible targets; no System outside Applications |

## Builtin resolver inventory

| Resolver family                 | Builtin responsibilities                                                                                                                | Source/selection rule                                                                                       |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Filesystem adapter / FsResolver | Enumeration, stat, bounded streams, write lifecycle and operation checks                                                                | Resolve owning local/USB/Core VFS/plugin connection/view adapter; never treat an opaque URI as a disk path  |
| Context menu                    | Opening, tags, create, connection cache, rename/batch rename, clipboard, terminal, copy path, Trash; VFS-specific launch/restore/delete | Common capabilities; Core VFS special actions and plugin contributions share EventRouter/session validation |
| Decorations                     | Tags, explicit readonly, connection cache/upload, application icons/provenance                                                          | Entry metadata and current view; no blanket readonly or System inference                                    |
| Info                            | Metadata/media sections plus approved provider sections                                                                                 | Resource metadata first; content via owning adapter and approved scope                                      |
| Preview                         | Builtin candidates/session and approved plugin previews                                                                                 | Content read is separate from metadata; launch-only Shell identities have no file stream                    |
| Search                          | Builtin local/USB search and declared plugin providers                                                                                  | Requested resource roots; owning adapter controls content access                                            |
| Toolbar/hooks                   | Declared extensions, before-open and user-triggered toolbar actions                                                                     | Shared resource-ref scope and matching; no official-plugin privilege                                        |
| Connection menu/navigation      | Edit/save/autostart/close and VFS sync; plugin-owned actions/views                                                                      | Uses manifest/navigation results, never a hardcoded SSH plugin ID                                           |

Core contains only local PTY lifecycle (`desktop/terminal.rs`) and local shell discovery/startup. Remote SSH implementation, UI and connection schema belong to plugins. Core no longer registers `ssh_open/ssh_write/ssh_resize/ssh_close` or `host.terminal.*`, nor embeds `org.fiex.ssh` routing. Old builtin SSH records require a plugin connection; they are not silently migrated or given credentials. The generic connection owner decides VFS versus plugin-view navigation.

## Delivery status

The code paths and contract were reviewed statically. Per the user's explicit instruction, no tests, builds, typechecks, lint runs or CI waits were run for this change. Windows Shell image extraction and platform-specific Trash behavior still require the user's platform verification. This document is a behavior contract and implementation handoff, not a claim that runtime checks passed.
