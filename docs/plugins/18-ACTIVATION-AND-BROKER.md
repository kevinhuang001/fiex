# Activation and brokered resources

Installing a package does not activate it or grant its requested capabilities.
Selecting Activate opens a review of every declared capability, network host and
external command. Each item must be checked individually. The host compares the
submitted snapshot with the currently installed manifest and atomically grants
and activates it. Partial or stale consent is rejected. Removing any grant
disables the plugin; reactivation requires another complete review.

Native plugin processes never receive access to the user's directory, package
writes, direct sockets (including localhost) or subprocess execution. They retain
only runtime system reads and inherited control pipes. SDK calls are requests to
Core, not OS permissions; a network grant never weakens the sandbox.

## Manifest capabilities

```json
{
  "folders": {
    "read": { "enabled": true },
    "create": true,
    "delete": false,
    "rename": false
  },
  "files": { "read": true, "write": true },
  "network": { "enabled": true, "domains": ["example.com", "192.168.1.10"] },
  "commands": { "enabled": false, "allow": [] }
}
```

`network.domains` is the shared exact-host allowlist for HTTP(S) and TCP.
It includes IP literals. Approving a host permits valid ports on that host;
HTTP and TCP do not have separate grants. Localhost and LAN addresses require
explicit authorization too. HTTP redirects are checked at each hop, and
credentials are removed on an origin change. HTTPS downgrades are rejected.

File operations are bounded by the directory captured for the task and its
subdirectories. File writes bind their own destination and initial fingerprint
at begin-write time. Chunks are staged by Core; commit detects content changes
before publication. Dropping the SDK writer aborts its pending transaction.
Core confines path resolution with directory capabilities rather than relying
on canonicalize-then-open checks. The root cannot be deleted or renamed.

## External commands are an explicit trust boundary

An approved command is resolved through the host's PATH and runs as the current
user outside the plugin sandbox, with plugin-supplied argument arrays. It may
read or modify files outside the file SDK scope, connect to a network or start
other programs. The generic consent screen explains this for every command.
Command approval is not a promise that the executable obeys file/network grants.
There are no privileged official plugin IDs and no Git-specific host operations.

## High-level SDK

Use `host.files()` for ordinary reads, streaming readers and transactional
writers. Use `host.http()` for request builders, JSON and streaming HTTP
responses. Neither API exposes a shared-memory descriptor to plugin authors.
Non-HTTP libraries can use the host-brokered `Read`/`Write` TCP adapter on a
worker. Named shared-memory APIs remain legacy interfaces and are not the
transport used by the strictly isolated official plugins.

## Tests and repository ownership

Core tests use an independent SDK mock process. Business-specific integration
fixtures for official plugins belong to fiex-official-plugins/integration.
SDK serialization, adapters and browser bridge tests belong to fiex-sdk.
