---
name: review-with-memory
description: Memory-augmented code review — pre-search context, then save findings
---

When reviewing code changes:

1. **Pre-search**: Call `search_memory` with the module name, feature area, or PR title to retrieve prior decisions and conventions.
2. **Review**: Analyze the code with the retrieved context in mind. Flag deviations from established patterns or decisions.
3. **Save findings**: After the review, call `add_memory` to persist any new architectural decisions, conventions, or patterns discovered during review. Tag with source "code-review".

This ensures institutional knowledge accumulates with each review cycle.
