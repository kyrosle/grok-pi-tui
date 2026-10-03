---
name: grok-build-review
description: >-
  Review xai-org/grok-build as a reference repository. Transcribe Changes bullets,
  decide port/adapt/skip-business/observe, and record decisions without merging.
---

# Grok Build reference review

Apply the governing [SPEC](../../../docs/issues/架构/20261003-pi-native-tui-SPEC.md). Grok Build supplies reference UI ideas; Pi owns agent functionality. This skill reviews changes and never ports, merges or pushes them.

1. Confirm the project root and `grok-build` remote. Fetch `grok-build main` when a current reference review is requested; this changes only local tracking refs.
2. Read the first line of `docs/grok-build/REVIEWED` as the reviewed commit. Verify that it and `grok-build/main` exist and that the reviewed commit is an ancestor. Range is `REVIEWED..grok-build/main`, never the product merge-base. Equal tips mean no new review.
3. Read every commit body in the range, including `Changes:` and `Source-Revision:`. Transcribe Changes bullets as the primary evidence. Only use a diff-derived description if a commit lacks Changes, and label it as such.
4. Use changed paths to preclassify: agent/shell/tools/workspace/MCP/auth/login/voice/telemetry/memory/marketplace business changes are skip candidates; pager/render/markdown/diff/mermaid/TTY/textarea changes require UI assessment. A mixed UI/business change needs separation, not automatic acceptance. Deleted business paths stay skipped.
5. Decide each bullet: **Port**, **Adapt then port**, **Skip business**, or **Observe**. Give a concrete reason, existing target path and any Pi semantic dependency. Use [area map](references/area-map.md) for path orientation, not as an acceptance rule.
6. Append a structured entry to `docs/grok-build/REVIEW_LOG.md` using [template](assets/entry-template.md). Record exact SHAs, source revisions, range and decisions. Do not claim a port until its actual commit exists.
7. Once every change in the range has a recorded decision, update REVIEWED: first line full reference SHA, second line `Source-Revision: <full revision>`. This advances reviewed coverage, not adoption or source-sync status.

Use only Git read/fetch operations and review-document writes. No merge, reset, checkout, rebase, push, business data changes or production source changes. Actual ports are separate worktree tasks and keep licensing/copyright plus `Ported-From: grok-build <sha> (Source-Revision <rev>)` trailers.
