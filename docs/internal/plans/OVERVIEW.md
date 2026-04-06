# Prototype Launch Plan — Overview

**Branch:** `feat/prototype-v1`
**Goal:** Public GitHub release of Context Keeper v0.1.0

## Plan Levels

```
Level 0: Foundation Sprint     (1-2 days)  ← START HERE
    ↓
Level 1: Core Correctness      (3-5 days)
    ↓
Level 2: Publishing & Release   (2-3 days)
    ↓
Level 3: Post-Launch            (2-4 weeks, reference only)
```

## Level 0 — Foundation Sprint
Architecture prerequisites from ADR-001 that must land before correctness work.
- 0.1: Typed error hierarchy (`ContextKeeperError` enum)
- 0.2: LLM extraction retry + validation
- 0.3: Composite entity identity (name + type)

**Plan file:** `docs/plans/LEVEL-0-foundation.md`

## Level 1 — Core Correctness
Fix the two issues that would break a public demo.
- 1.1: Memory updates / negation detection (FZ-12)
- 1.2: Entity relationship quality (FZ-13)
- 1.3: Expand entity types — light pass (FZ-14)

**Plan file:** `docs/plans/LEVEL-1-correctness.md`

## Level 2 — Publishing & Release
Everything to ship v0.1.0.
- 2.1: Working examples
- 2.2: CI/CD pipeline
- 2.3: README polish
- 2.4: crates.io prep
- 2.5: Docker Hub publishing
- 2.6: License & legal

**Plan file:** `docs/plans/LEVEL-2-publish.md`

## Level 3 — Post-Launch (Reference)
Benchmarking, demos, modes, plugins, local models.
**Plan file:** `docs/plans/LEVEL-3-post-launch.md`

## Linear Issue Mapping

| Task | Linear Issue | Priority | Milestone |
|------|-------------|----------|-----------|
| 0.1 Typed errors | [FZ-59](https://linear.app/0x319/issue/FZ-59) | Urgent | Efficacy & Correctness |
| 0.2 Extraction retry | [FZ-57](https://linear.app/0x319/issue/FZ-57) | Urgent | Efficacy & Correctness |
| 0.3 Composite entity ID | [FZ-58](https://linear.app/0x319/issue/FZ-58) | High | Efficacy & Correctness |
| 1.1 Memory updates | [FZ-12](https://linear.app/0x319/issue/FZ-12) | Urgent | Efficacy & Correctness |
| 1.2 Entity relations | [FZ-13](https://linear.app/0x319/issue/FZ-13) | Medium | Efficacy & Correctness |
| 1.3 Entity types | [FZ-14](https://linear.app/0x319/issue/FZ-14) | Low | Efficacy & Correctness |
| 2.1 Examples | [FZ-60](https://linear.app/0x319/issue/FZ-60) | High | Efficacy & Correctness |
| 2.2 CI/CD | [FZ-61](https://linear.app/0x319/issue/FZ-61) | High | Efficacy & Correctness |
| 2.3 README polish | [FZ-62](https://linear.app/0x319/issue/FZ-62) | Medium | Efficacy & Correctness |
| 2.4 crates.io prep | [FZ-64](https://linear.app/0x319/issue/FZ-64) | Medium | Efficacy & Correctness |
| 2.5 Docker Hub | [FZ-63](https://linear.app/0x319/issue/FZ-63) | Medium | Efficacy & Correctness |
| 2.6 License & legal | [FZ-65](https://linear.app/0x319/issue/FZ-65) | Low | Efficacy & Correctness |

## How to Delete Plan Files

Each level file includes a "Delete This File When" section. Once a level is complete, delete its plan file and update this overview. When all levels are done, delete the entire `docs/plans/` directory.
