---
name: save-session-context
description: Capture session decisions, learnings, and trade-offs to memory
---

At the end of a work session (or when the user asks to save context), persist the most important information:

1. Identify key decisions made during the session.
2. Note any trade-offs, rejected approaches, or open questions.
3. Call `add_memory` for each distinct piece of information, using a descriptive `source` tag (e.g. "session-2024-03-15", "code-review", "architecture-decision").
4. Confirm what was saved by listing the entities extracted.

Keep memory entries focused — one concept per call. Prefer specific facts over vague summaries.
