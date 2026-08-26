# Task for crew-planner

Create a task breakdown for implementing this request.

## Request

Create exactly one ready Crew task for OpenSpec change migrate-tui-to-tuirealm: nested row 5.3d.8 only, Audiobookshelf podcast downstream-reader/cover-fetch discovery. Starting from clean HEAD b8849afe. Objective: inspect the current source and enumerate persistence, queue-target, legacy-render, image-fetch, layout, and remaining interaction-field readers; write a durable symbol-level report at openspec/handoffs/scout-abs-podcast-teardown.md; then refine tasks.md rows 5.3d.9-5.3d.11 into explicit dependency-ordered implementation subrows, each bounded to roughly 3-6 production files. No production code edit. Do not implement 5.3d.9+. Do not alter unrelated planning artifacts. Commit the report plus tasks.md as one new commit; do not amend or push; report SHA and exact files. The task spec must name CONTEXT.md, design D17, ADR 0002, ADR 0021, ADR 0022, the VividYak handoff, and the existing podcast scout as required context, but instruct the worker to inspect only symbols required for this report rather than broadly rereading the repository. Verification: show the reader/writer inventory with exact symbols and paths, justify every bounded implementation row and dependency, confirm no production files changed, and run formatting only if applicable to markdown (do not run cargo checks for a doc-only scout). Use one worker only; later implementation and review will be separate tasks.

You must follow this sequence strictly:
1) Understand the request
2) Review relevant code/docs/reference resources
3) Produce sequential implementation steps
4) Produce a parallel task graph

Return output in this exact section order and headings:
## 1. PRD Understanding Summary
## 2. Relevant Code/Docs/Resources Reviewed
## 3. Sequential Implementation Steps
## 4. Parallelized Task Graph

In section 4, include both:
- markdown task breakdown
- a `tasks-json` fenced block with task objects containing title, description, and dependsOn.