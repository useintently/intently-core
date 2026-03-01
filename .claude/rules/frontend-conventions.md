# Frontend Conventions

Non-negotiable frontend conventions for the Intently IDE desktop application.

## Stack
- React 18+ with TypeScript in strict mode (`"strict": true` in tsconfig)
- Vite as the build tool and dev server
- Tauri v2 for desktop shell and native API access
- All frontend code lives in `apps/desktop/src/`

## TypeScript
- ALL functions MUST have explicit parameter and return type annotations
- Use `interface` for object shapes, `type` for unions and intersections
- NEVER use `any` — use `unknown` and narrow with type guards
- Enable strict null checks — handle `null | undefined` explicitly
- Prefer `const` over `let`; never use `var`

## Components
- Functional components only — no class components
- Extract reusable logic into custom hooks (`use*.ts`)
- One component per file, filename matches component name in PascalCase
- Co-locate component, styles, and tests: `Button.tsx`, `Button.test.tsx`
- Props interfaces named `{ComponentName}Props`

## State Management
- Use `zustand` or `jotai` for global state (lightweight, no boilerplate)
- Local state: `useState` and `useReducer`
- NEVER store derived state — compute it
- Keep stores small and focused (one per domain area)

## Tauri IPC
- All Tauri commands invoked via `@tauri-apps/api/core` invoke function
- Type all command inputs and outputs — use generated bindings when available
- Handle IPC errors explicitly — never ignore rejected promises
- NEVER pass raw user input directly to shell commands
- Use Tauri event system (`listen`, `emit`) for core-to-frontend communication

## Styling
- Tailwind CSS for all styling
- No inline styles except for truly dynamic values (e.g., computed positions)
- Extract repeated patterns into Tailwind `@apply` classes or component abstractions
- Dark mode support required — use Tailwind dark: variants

## Testing
- Vitest as the test runner (aligned with Vite)
- React Testing Library for component tests — test behavior, not implementation
- Coverage target: 80% for `src/` (excluding generated files)
- Mock Tauri invoke calls in tests using `vi.mock`
- Test names describe behavior: `it("disables submit when form is invalid")`

## File Naming
- PascalCase: React components (`IntentEditor.tsx`)
- camelCase: hooks (`useSession.ts`), utilities (`parseIntent.ts`), stores (`sessionStore.ts`)
- kebab-case: CSS/config files, route paths
- `index.ts` files ONLY for barrel exports from feature directories

## Imports
- Group: react → third-party → @tauri → local, separated by blank lines
- Absolute imports via `@/` alias mapped to `src/`
- NEVER use `import *` — always named imports
- Prefer tree-shakeable imports

## Performance
- Use `React.memo` only when profiling shows re-render problems — not preemptively
- Lazy load routes and heavy components with `React.lazy` + `Suspense`
- Debounce expensive operations (search, resize, scroll handlers)
- Avoid creating objects/arrays in render — use `useMemo` when identity matters

## Accessibility
- All interactive elements must be keyboard accessible
- Use semantic HTML elements (`button`, `nav`, `main`, `section`)
- Include `aria-label` on icon-only buttons
- Maintain visible focus indicators
