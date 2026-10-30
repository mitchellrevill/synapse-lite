# Compatibility

The inherited device database lists hardware facts and capability declarations. It does not certify every model, firmware version, regional layout or connection mode. Product artwork, editable LED geometry, a protocol implementation and a physical test are separate coverage requirements.

| Area | Current boundary |
| --- | --- |
| Windows | Native desktop app and CLI. |
| Linux | CLI; native desktop UI is not available. |
| Device settings | Implemented per capability and transport. Unsupported operations must remain unavailable. |
| Native profiles | Local backups/imports with explicit device/profile selection. |
| Synapse 4 profiles | Supported payloads retain opaque fields; unsupported conversions are reported or refused. Full re-import acceptance is not established. |
| Synapse 3 ZIP profiles | Archive inspection exists; full profile translation is incomplete. |
| Studio | Editing and supported rendering; physical output currently admits a narrow authenticated keyboard/layout path. |
| Macros and remapping | Device/driver-dependent; authoring support is broader than verified live execution. |

## Device identity

A public product ID, dongle ID or matching image is insufficient to select a physical layout. Preserve product, edition, regional layout and connection roles separately. Ambiguous identity must not select the first available match.

`secbird catalog inspect FILE` inspects a local public catalogue without opening hardware. The collection and asset-audit tools preserve those identity roles and report missing resources. They do not infer firmware commands from photographs or claim all-market support.

## Reporting a device

Provide `secbird report`, build version, firmware, connection mode and the operation that failed. Redact personal information before sharing. Keep unverified devices in the database where their facts remain valid; diagnostic cleanup must not remove actual support.
