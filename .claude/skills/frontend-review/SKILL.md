# Frontend Review

Frontend (React + TypeScript) code review for the Intently IDE desktop application.

## Trigger

Activate when PRs or changes touch:
- `apps/desktop/src/` (React/TypeScript files)
- Component files (`.tsx`), hooks, stores, or utilities
- Frontend configuration (`tsconfig.json`, `vite.config.ts`)

Keywords: "frontend review", "review frontend", "react review", "review component", "UI review"

## What This Skill Does

1. **TypeScript Strict Mode** — Verify type safety
   - `strict: true` in `tsconfig.json`
   - No `any` type (use `unknown` and narrow with type guards)
   - No `@ts-ignore` or `@ts-expect-error` without justification
   - Function return types are explicit for public APIs

2. **Component Patterns** — Review React component design
   - Components are small and focused (SRP)
   - Props interfaces are explicit and documented
   - No prop drilling beyond 2 levels (use context or state management)
   - Controlled vs uncontrolled components used appropriately

3. **Hooks Usage** — Validate React hooks
   - `useEffect` has correct dependency arrays (no missing deps)
   - `useMemo`/`useCallback` used for expensive computations or stable references
   - Custom hooks extract reusable stateful logic
   - No hooks called conditionally or in loops

4. **Tauri Invoke Typing** — Check IPC type safety
   - `invoke<T>()` calls have explicit type parameters
   - Return types match the Rust command signatures
   - Error handling for invoke failures (network, serialization, command errors)
   - No string-typed command names (use constants or generated types)

5. **Accessibility** — Verify basic a11y
   - Interactive elements have keyboard support
   - Images have alt text, icons have aria-labels
   - Color is not the only indicator of state
   - Focus management for modals and dialogs

6. **Performance** — Check rendering efficiency
   - Large lists use virtualization (react-window or similar)
   - Graph visualizations handle large node counts
   - No unnecessary re-renders (React DevTools profiler clean)
   - Lazy loading for heavy components/routes

7. **Testing** — Verify test coverage
   - Components have Vitest + Testing Library tests
   - Tests verify behavior, not implementation details
   - User interactions are tested (click, type, submit)
   - Edge cases: loading states, error states, empty states

## What to Check

- [ ] TypeScript strict mode, no `any` types
- [ ] Components are small, props interfaces explicit
- [ ] Hooks have correct dependency arrays
- [ ] Tauri invoke calls are typed
- [ ] Basic accessibility requirements met
- [ ] Large datasets use virtualization
- [ ] Tests cover behavior and user interactions

## Output Format

```
## Frontend Review: <file_path>

### Type Safety
- [PASS/FAIL] <detail>

### Component Design
- [PASS/FAIL] <detail>

### Hooks
- [PASS/FAIL] <detail>

### Tauri IPC
- [PASS/FAIL] <detail>

### Accessibility
- [PASS/FAIL] <detail>

### Verdict: APPROVE / REQUEST_CHANGES / NEEDS_DISCUSSION
```
