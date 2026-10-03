# T0 crate ownership inventory

Baseline `7bbd748a`; one locked Pi production normal/build graph: `xai-grok-pager-bin --no-default-features --features jemalloc,sandbox-enforce`.

796 resolved packages; SPEC classes cover 87 workspace crates, 56 linked and 31 absent, including the Root-confirmed xai-mixpanel exporter addendum. Reverse parents are reconstructed from the actual depth tree. Rust references include stock/test code and indicate coupling, not runtime execution.

| Crate | Initial class | Production linked | Resolved direct parents | Direct surface refs | Decision / extraction boundary |
|---|---|---|---|---:|---|
| `pi-grok-adapter` | 保留：基础设施 | yes | xai-grok-pager-bin | 4 | linked_retained_ui_or_infrastructure |
| `xai-acp-lib` | 保留：基础设施 | yes | pi-grok-adapter, xai-grok-pager, xai-grok-pager-bin | 191 | linked_retained_ui_or_infrastructure |
| `xai-circuit-breaker` | 待定 | yes | xai-file-utils, xai-grok-sampling-types | 0 | linked_transitive_pending_ownership |
| `xai-codebase-graph` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-compaction-transcript` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-computer-hub-core` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-computer-hub-mcp-adapter` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-computer-hub-sdk` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-crash-handler` | 保留：基础设施 | yes | xai-grok-pager, xai-grok-pager-bin | 14 | linked_retained_ui_or_infrastructure |
| `xai-dirs` | 保留：基础设施 | yes | xai-fast-worktree, xai-file-utils, xai-grok-config, xai-grok-foreign-sessions, xai-grok-pager, xai-grok-pager-render, xai-grok-sandbox, xai-grok-shared, xai-grok-shell-base, xai-grok-telemetry, xai-grok-update, xai-grok-voice | 32 | linked_retained_ui_or_infrastructure |
| `xai-fast-worktree` | 待定 | yes | xai-grok-pager | 19 | Direct Pager reference currently constants (ENOSPC) plus test/headless stock registry calls; xai-gix-status transitive through this crate. Not proof that Pi runs native worktree creation. |
| `xai-file-utils` | 保留：基础设施 | yes | xai-grok-login, xai-grok-pager, xai-grok-telemetry | 2 | linked_retained_ui_or_infrastructure |
| `xai-fsnotify` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-fuzzy-file-search` | 保留：TUI核心 | yes | xai-grok-pager | 6 | linked_retained_ui_or_infrastructure |
| `xai-gix-status` | 待定 | yes | xai-fast-worktree | 0 | linked_transitive_pending_ownership |
| `xai-grok-active-sessions` | 待定 | yes | xai-grok-pager, xai-grok-pager-bin | 9 | Direct shared Effect RegisterActiveSession and signal unregister, plus stock bin crash notice. It remains called from common session-load replay; classify mixed local UI lifecycle/stock product registry. |
| `xai-grok-agent` | 移除：Grok业务 | no | — | 18 | stock_only_in_this_profile |
| `xai-grok-announcements` | 移除：Grok业务 | yes | xai-grok-config-types, xai-grok-pager, xai-grok-shared | 107 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-auth` | 移除：Grok业务 | yes | xai-file-utils, xai-grok-http, xai-grok-login, xai-grok-otel, xai-grok-telemetry | 0 | linked_business_transitive |
| `xai-grok-bundle` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-compaction` | 待定 | yes | xai-grok-sampling-types | 0 | Transitive through xai-grok-sampling-types only in this profile; no direct Pager/adapter use. Pi owns compaction; removal follows shared sampling-type extraction. |
| `xai-grok-config` | 保留：基础设施 | yes | pi-grok-adapter, xai-grok-active-sessions, xai-grok-announcements, xai-grok-config-types, xai-grok-dashboard-store, xai-grok-hooks, xai-grok-login, xai-grok-pager, xai-grok-pager-bin, xai-grok-pager-render, xai-grok-sandbox, xai-grok-shared, xai-grok-shell-base, xai-grok-telemetry, xai-grok-update | 161 | linked_retained_ui_or_infrastructure |
| `xai-grok-config-types` | 保留：基础设施 | yes | xai-fast-worktree, xai-grok-login, xai-grok-pager, xai-grok-shared, xai-grok-telemetry, xai-grok-workspace-types | 39 | linked_retained_ui_or_infrastructure |
| `xai-grok-dashboard-store` | 移除：Grok业务 | yes | xai-grok-pager | 230 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-diag-server` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-extra-ca` | 保留：基础设施 | yes | xai-file-utils, xai-grok-hooks, xai-grok-http, xai-grok-otel, xai-grok-pager-bin, xai-grok-shell-base, xai-grok-telemetry, xai-grok-update, xai-grok-voice | 2 | linked_retained_ui_or_infrastructure |
| `xai-grok-feedback` | 移除：Grok业务 | yes | xai-grok-pager, xai-grok-shared | 25 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-foreign-sessions` | 待定 | yes | xai-grok-pager | 41 | Direct Pager SourceFilter/session discovery and config type use. Pi/foreign page reuse is mixed; T1 now separates stock External filter from Pi PSM load/result acceptance. |
| `xai-grok-gboom` | 移除：Grok业务 | yes | xai-grok-pager, xai-grok-pager-render | 3 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-hooks` | 待定 | yes | xai-grok-pager | 3 | Pager directly uses hook_display_name/HookDisplayName for blocked-card/completion presentation. Extract display-name types; native hooks execution stays Grok business. |
| `xai-grok-image` | 保留：TUI核心 | yes | xai-grok-pager-render, xai-grok-shared | 1 | Retain: source is local image bytes validation/transcoding, no service client. Cargo has image/crc32fast/thiserror only. |
| `xai-grok-login` | 移除：Grok业务 | yes | xai-grok-pager, xai-grok-update | 82 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-markdown` | 保留：TUI核心 | yes | xai-grok-pager, xai-grok-pager-render | 34 | linked_retained_ui_or_infrastructure |
| `xai-grok-markdown-core` | 保留：TUI核心 | yes | xai-grok-markdown | 0 | linked_retained_ui_or_infrastructure |
| `xai-grok-mcp` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-memory` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-mermaid` | 保留：TUI核心 | yes | xai-grok-pager | 5 | linked_retained_ui_or_infrastructure |
| `xai-grok-models` | 移除：Grok业务 | yes | xai-grok-pager, xai-grok-shared | 6 | Mixed defaults/model catalog references in Pager/shared. Pi models remain authoritative; migrate fallback types/defaults before removal. |
| `xai-grok-otel` | 移除：Grok业务 | yes | xai-file-utils, xai-grok-pager, xai-grok-telemetry | 1 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-pager` | 保留：TUI核心 | yes | xai-grok-pager-bin, xai-grok-pager-minimal | 264 | linked_retained_ui_or_infrastructure |
| `xai-grok-pager-bin` | 保留：基础设施 | yes | — | 0 | linked_retained_ui_or_infrastructure |
| `xai-grok-pager-diff` | 保留：TUI核心 | yes | xai-grok-pager, xai-grok-pager-minimal | 23 | linked_retained_ui_or_infrastructure |
| `xai-grok-pager-minimal` | 保留：TUI核心 | yes | xai-grok-pager-bin | 4 | linked_retained_ui_or_infrastructure |
| `xai-grok-pager-pty-harness` | 保留：基础设施 | no | — | 0 | test_support_only |
| `xai-grok-pager-render` | 保留：TUI核心 | yes | xai-grok-pager | 17 | linked_retained_ui_or_infrastructure |
| `xai-grok-paths` | 保留：基础设施 | yes | xai-grok-pager-render | 5 | linked_retained_ui_or_infrastructure |
| `xai-grok-plugin-marketplace` | 移除：Grok业务 | no | — | 54 | stock_only_in_this_profile |
| `xai-grok-sampler` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-sampling-types` | 移除：Grok业务 | yes | xai-grok-config-types, xai-grok-http, xai-grok-login, xai-grok-pager, xai-grok-shared, xai-grok-telemetry | 20 | Mixed DTOs: Pager Actions/ACP use ReasoningEffort; config/shared/login/http/telemetry also depend on it. Removing this crate requires neutral type owner and cuts its compaction chain. |
| `xai-grok-sandbox` | 待定 | yes | xai-grok-hooks, xai-grok-pager, xai-grok-pager-bin | 12 | Linked but no direct reference in grok-pi.rs/grok_pi or adapter. Pager sandbox requested_confinement runs in stock app::run. sandbox-enforce feature reaches dependency but does not establish confinement of tools executed by Pi. T2 decision needs runtime design, not graph-only proof. |
| `xai-grok-secrets` | 移除：Grok业务 | yes | xai-grok-otel, xai-grok-telemetry, xai-mixpanel | 0 | linked_business_transitive |
| `xai-grok-session-events` | 待定 | yes | xai-grok-shared, xai-grok-telemetry | 0 | Transitive shared/telemetry DTO owner. No direct surface :: references detected. Split the generic event contract from stock telemetry. |
| `xai-grok-session-search` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-shared` | 保留：基础设施 | yes | xai-grok-pager, xai-grok-pager-bin, xai-grok-pager-minimal, xai-grok-pager-render, xai-grok-shell-base | 486 | linked_retained_ui_or_infrastructure |
| `xai-grok-shell` | 移除：Grok业务 | no | — | 344 | stock_only_in_this_profile |
| `xai-grok-shell-base` | 移除：Grok业务 | yes | xai-grok-login, xai-grok-pager, xai-grok-update | 8 | Mixed: direct Pager tips/changelog DTO/util references, plus login/update parents. Extract generic UI helpers before removing shell*. |
| `xai-grok-shell-session-support` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-shell-terminal` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-status-line` | 保留：TUI核心 | yes | xai-grok-pager, xai-grok-shared | 20 | linked_retained_ui_or_infrastructure |
| `xai-grok-telemetry` | 移除：Grok业务 | yes | xai-grok-http, xai-grok-login, xai-grok-pager, xai-grok-pager-bin, xai-grok-pager-render, xai-grok-update | 371 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-test-support` | 保留：基础设施 | no | — | 35 | test_support_only |
| `xai-grok-tools` | 移除：Grok业务 | no | — | 62 | stock_only_in_this_profile |
| `xai-grok-tools-api` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-update` | 保留：基础设施 | yes | xai-grok-pager, xai-grok-pager-bin | 17 | linked_retained_ui_or_infrastructure |
| `xai-grok-version` | 保留：基础设施 | yes | xai-file-utils, xai-grok-config, xai-grok-http, xai-grok-login, xai-grok-pager, xai-grok-pager-bin, xai-grok-pager-minimal, xai-grok-shared, xai-grok-shell-base, xai-grok-telemetry, xai-grok-update | 28 | linked_retained_ui_or_infrastructure |
| `xai-grok-voice` | 移除：Grok业务 | yes | xai-grok-pager | 88 | linked_business_with_ui_or_shared_refs_requires_split |
| `xai-grok-workspace` | 移除：Grok业务 | no | — | 30 | stock_only_in_this_profile |
| `xai-grok-workspace-client` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-workspace-daemon` | 移除：Grok业务 | no | — | 0 | stock_only_in_this_profile |
| `xai-grok-workspace-types` | 移除：Grok业务 | yes | xai-grok-pager | 20 | Mixed shared DTOs: Pager Actions use RestoreDegree/git/workspace request types; workspace runtime itself absent. Extract neutral contracts before wildcard deletion. |
| `xai-hooks-plugins-types` | 待定 | yes | xai-grok-pager, xai-grok-shared | 351 | Mixed display/protocol types plus stock plugin management references. Broad deletion requires extracting shared presentation DTOs. |
| `xai-hunk-tracker` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-interjection-core` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-message-delivery-core` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-prompt-queue` | 待定 | yes | xai-grok-pager | 10 | Direct Pager agent enqueue combination/join algorithm use. Queue ownership debt PI-01; not removable until Pi queue migration. |
| `xai-ratatui-inline` | 保留：TUI核心 | yes | xai-grok-pager, xai-grok-pager-minimal, xai-grok-pager-render | 29 | linked_retained_ui_or_infrastructure |
| `xai-ratatui-textarea` | 保留：TUI核心 | yes | xai-grok-pager, xai-grok-pager-render | 49 | linked_retained_ui_or_infrastructure |
| `xai-test-utils` | 待定 | no | — | 0 | not_linked_in_this_profile |
| `xai-token-estimation` | 保留：基础设施 | yes | xai-grok-pager, xai-grok-pager-minimal, xai-grok-shared, xai-grok-telemetry | 5 | linked_retained_ui_or_infrastructure |
| `xai-tool-protocol` | 待定 | yes | xai-grok-shared, xai-grok-workspace-types | 0 | Transitive via shared and workspace-types; no direct surface references detected. Extract required neutral protocol DTOs during workspace/shared split. |
| `xai-tool-runtime` | 待定 | no | — | 0 | Not linked in locked Pi production profile; stock tool runtime candidate. |
| `xai-tool-types` | 待定 | yes | xai-grok-auth, xai-grok-hooks, xai-grok-http, xai-grok-login, xai-grok-pager, xai-grok-pager-diff, xai-grok-pager-minimal, xai-grok-sampling-types, xai-grok-shared, xai-tool-protocol, xai-workflow | 182 | Direct diff/Pager/minimal/Workflow/shared wire contracts. Retain or extract neutral UI DTOs; do not remove as a Grok tool runtime. |
| `xai-tracing` | 保留：基础设施 | no | — | 0 | Not linked in locked Pi production profile. Product uses standard tracing/tracing-subscriber; retaining xai-tracing* is not currently a product requirement. |
| `xai-tracing-macros` | 保留：基础设施 | no | — | 0 | Not linked in locked Pi production profile; follows xai-tracing. |
| `xai-tty-utils` | 保留：TUI核心 | yes | xai-fast-worktree, xai-grok-config, xai-grok-hooks, xai-grok-login, xai-grok-mermaid, xai-grok-pager, xai-grok-pager-bin, xai-grok-pager-render, xai-grok-shared, xai-grok-shell-base, xai-grok-telemetry, xai-grok-update, xai-grok-voice, xai-workflow | 62 | linked_retained_ui_or_infrastructure |
| `xai-workflow` | 待定 | yes | pi-grok-adapter | 14 | Direct adapter use: pi_workflow_backend/workflow_host, product opt-in Rhai runtime. Ownership debt EX-08; not stock-only. |

| `xai-mixpanel` | 移除：Grok 业务 / telemetry exporter | yes | xai-grok-telemetry | transitive | Cargo tree line 2705 and telemetry Cargo.toml:36 confirm the export dependency; remove with telemetry in T2. |

Full captured graph and detailed source-reference receipt: `/tmp/grok-pi-native-t0-production-tree-20261003.txt`, `/tmp/grok-pi-native-t0-classification-20261003.json`. These are stage evidence, not new source-identity constraints.

Removal ordering must extract neutral UI contracts from shell-base/workspace-types/sampling-types/models before cutting their runtime dependencies. Sandbox/hook/foreign-session/workflow decisions require the specified later stage and actual behavior evidence.

The detailed JSON covers the original 86 entries; the xai-mixpanel row is an explicit addendum confirmed against the same 796-package graph. Pending removal policy now names only the 29 already-linked packages; already-absent and newly introduced business dependencies fail in every phase.
