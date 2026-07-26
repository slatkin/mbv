# Testing Rules

Tests are not a measure of how much work was done. Do not add tests by default,
and do not aim for a test count or coverage percentage.

## When to add a test

Add a test when it is likely to catch a realistic problem that the existing
suite would miss. Good reasons include:

- protecting behavior that previously broke;
- protecting saved data or compatibility with older versions;
- protecting a network, process, filesystem, or other external boundary;
- checking complicated logic where a mistake would be hard to notice; or
- checking an important failure path that must not lose or corrupt state.

A bug fix does not automatically need a new test. First confirm the actual
failure and the code path causing it. Add a test only if it gives useful,
lasting protection against that failure happening again.

## Before writing a test

1. Search for tests that already cover the behavior.
2. Prefer improving an existing test over adding another test beside it.
3. Ask whether the test would catch a realistic mistake, rather than merely
   repeating the implementation.
4. Use the smallest test that proves the behavior.
5. If several inputs prove the same point, use one table of cases instead of
   separate test functions.

## Tests to avoid

Do not add tests merely to:

- cover every function, branch, input, or edge case;
- test getters, simple formatting, obvious conversions, or library behavior;
- assert private fields or other implementation details;
- repeat behavior already covered by a stronger test;
- satisfy a plan item that only says "add tests" without explaining why; or
- replace direct or visual verification of user-facing behavior.

Do not create large fixtures or mocks unless the behavior being protected is
worth their maintenance cost.

## Keeping the suite useful

When changing an area, look for tests that can be combined, simplified, or
removed. Delete a test when another test proves the same behavior at least as
well. Keeping every old test is not a goal.

Each test should have a clear answer to this question:

> What realistic problem would this test catch that another test would not?

If there is no good answer, do not add the test.

## Verification

Testing is only one kind of evidence. Use the narrowest useful checks for the
change. Manual reproduction, logs, rendered output, and direct inspection may
be better evidence than another unit test. A change with no new tests is
acceptable when the existing suite already covers it or a new test would add
little value.
