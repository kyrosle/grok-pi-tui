# Grok Build reference review

Grok Build is a reference repository. Reviews record decisions, not merges or source identity. Read the first line of `REVIEWED` as the review watermark; its second line records the monorepo Source-Revision.

## Adopted review checkpoint — 2026-10-03

- Last reviewed reference commit: `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`.
- Source-Revision: `559751fdcec02d413e4c57c8832ab275e4f44980`.
- Evidence: [historical change transcription](archive/UPSTREAM_CHANGELOG.md) and [selective UI port record](../upstream/TUI_BACKPORTS.md).
- This is the previously reviewed range endpoint. T0 performed no new fetch, full merge or new port.

## Review entry format

Record the exact range, commit SHA and Source-Revision; preserve each Changes bullet, then classify it with a reason and target path. Every decision must be recorded before advancing REVIEWED. A completed review does not require accepting every change.

| Change | Decision | Reason | Target path | Port commit |
|---|---|---|---|---|
| Reference Changes bullet | Port / Adapt then port / Skip business / Observe | Concrete Pi/UI reason | Existing component, or none | Actual commit, or pending |

Actual ports use a separate worktree and a focused commit with `Ported-From: grok-build <sha> (Source-Revision <rev>)`. Do not merge the reference root.
