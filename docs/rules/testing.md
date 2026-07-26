# Testing Rules

## Minimum Test Coverage: 80%

Test Types (ALL required):
1. **Unit Tests** - Individual functions, utilities, components
2. **Integration Tests** - API endpoints, database operations
3. **E2E Tests** - Critical user flows

## Test-Driven Development

Not the default — full red/green/refactor TDD costs extra agent turns and tokens.
Write tests alongside or after implementation unless:
- the user explicitly asks for TDD, or
- a bug fix benefits from a reproduction test written before the fix, or
- an existing plan/spec the user already approved prescribes TDD (follow it as authored, don't fight it).

If skipping the red phase, still write tests thorough enough to have caught the bug.

## Edge Cases to Test

Every function must be tested with:
- [ ] Null/undefined inputs
- [ ] Empty arrays/strings
- [ ] Invalid types
- [ ] Boundary values (min/max)
- [ ] Error conditions

## Test Quality Checklist

- [ ] Tests are independent (no shared state)
- [ ] Test names describe behavior
- [ ] Mocks used for external dependencies
- [ ] Both happy path and error paths tested
- [ ] No flaky tests

## [CUSTOMIZE] Project-Specific Testing

Add your project-specific testing requirements here:
- Test framework configuration
- Mock setup patterns
- E2E test scenarios
