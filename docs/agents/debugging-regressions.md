# Regression Debugging Protocol

Use this protocol before editing code or writing tests for a reported regression.

## Required Sequence

1. State the product symptom in concrete user-visible terms.
2. Identify the active runtime path by reproduction, logs, tracing, or temporary instrumentation.
3. Confirm that the proposed edit is on that active path.
4. Compare recent changes only after the active path is known.
5. Do not write tests until the active path and product failure are confirmed.
6. Remove temporary instrumentation before finalizing the fix.

## Guardrails

- Do not turn an unverified hypothesis into a test.
- Do not treat unit tests as proof that the product symptom is fixed.
- Prefer short, targeted runtime logs over broad speculative edits.
- If the active path cannot be confirmed locally, stop and report exactly what evidence is missing.

## Success Criteria

A regression fix is ready for normal validation only after the user-visible failure has been reproduced or otherwise confirmed on the exact code path being changed.
