# Local File Commands

## Goal

Give `mixio-cli` CLI-native access to the two local-file operations already provided by the stdio MCP bridge: uploading a local file into Mixio media and resolving a local file to its cached/public URL.

## User-facing interface

The commands use the CLI's noun-first convention:

```text
mixio file upload <PATH> [--project-id ID] [--organization-id ID] [--alt TEXT] [--category CATEGORY] [--force]
mixio file url <PATH> [--project-id ID] [--organization-id ID] [--alt TEXT] [--category CATEGORY] [--no-upload]
```

`file upload` performs the stdio bridge's `upload_file` behavior. `file url` performs `get_public_url` behavior: it uploads on a cache miss unless `--no-upload` is supplied. Both commands emit JSON matching the stdio result shape so scripts can consume them without parsing human prose.

## Constraints

- These are static CLI commands, not hosted MCP tools: the hosted server cannot read a path on the CLI user's filesystem.
- Preserve the existing dynamic `mixio call` and derived noun/verb commands unchanged.
- Use the same Inference Files small-upload and presigned-upload flow as the stdio bridge, then associate the file with Studio through `/api/media/inference`.
- Cache entries per CLI profile and reuse them only when the local file SHA-256 and requested project/organization scope match.
- Do not print or persist API keys outside the existing profile credential store.
