# ADR-001: Extractor Gaps — Real-World Validation Results

**Date:** 2026-02-28
**Status:** Accepted
**Context:** Real-world validation of Intently MCP against 8 GitHub repositories across 6 languages revealed extraction gaps in 5 areas.

---

## Validation Summary

| Repo | Language | Framework | Files | Endpoints | Models | Refs | Symbols | Time |
|---|---|---|---|---|---|---|---|---|
| Conduit API (local) | TypeScript | Express | 39 | 21 | 11 | 520 | 12 | 146ms |
| tiangolo/full-stack-fastapi-template | Python | FastAPI | 44 | 23 | 22 | 880 | 141 | 77ms |
| gothinkster/golang-gin-realworld | Go | Gin | 19 | 31 | 29 | 1,672 | 138 | 53ms |
| lujakob/nestjs-realworld | TypeScript | NestJS | 36 | **0** | 36 | 566 | 1 | 101ms |
| spring-projects/spring-petclinic | Java | Spring Boot | 47 | 17 | 47 | 1,201 | 0 | 32ms |
| django/djangoproject.com | Python | Django | 249 | 114 | 378 | 9,671 | 1,182 | 250ms |
| dotnet/eShop | C# | ASP.NET | 538 | 14 | 465 | 6,378 | 2 | 911ms |
| tokio-rs/axum | Rust | Axum | 296 | 0 | 419 | 10,351 | 2,047 | 219ms |
| laravel/laravel | PHP | Laravel 11 | 30 | 1 | 0 | 332 | 0 | 37ms |

---

## GAP-01: NestJS Decorator-Based Routing (0 endpoints detected)

**Severity:** High
**Impact:** NestJS is the most popular Node.js framework by npm downloads. Zero endpoint detection means Intently is blind to all NestJS projects.
**File:** `crates/intently_core/src/twin/extractors/typescript.rs`

### Problem

The TypeScript extractor only handles Express-style `call_expression` patterns (`router.get('/path', handler)`). NestJS uses TypeScript decorators:

```typescript
@Controller('articles')
export class ArticleController {
  @Get(':slug')
  @UseGuards(AuthGuard('jwt'))
  async findOne(@Param('slug') slug: string) { ... }
}
```

The extractor's `extract_recursive` only matches `call_expression` and `import_statement` — it completely ignores `decorator`, `class_declaration`, and `method_definition` nodes.

### Missing Patterns

| Pattern | CST Node | HTTP Method | Path |
|---|---|---|---|
| `@Get()` | `decorator` → `call_expression(identifier:"Get")` | GET | (from `@Controller` prefix) |
| `@Get('feed')` | `decorator` → `call_expression(identifier:"Get", arguments:("feed"))` | GET | `/feed` |
| `@Post(':slug/comments')` | same structure, identifier `"Post"` | POST | `/:slug/comments` |
| `@Put(':slug')` | same structure, identifier `"Put"` | PUT | `/:slug` |
| `@Delete(':slug')` | same structure, identifier `"Delete"` | DELETE | `/:slug` |
| `@Patch(':slug')` | same structure, identifier `"Patch"` | PATCH | `/:slug` |
| `@Controller('articles')` | class-level decorator defining route prefix | - | `/articles` |
| `@UseGuards(AuthGuard('jwt'))` | auth guard on method or class | - | auth detection |

### Required Changes

1. **Add `class_declaration` handling to `extract_recursive`** — when encountered, collect class-level decorators for `@Controller('prefix')` and `@UseGuards`.

2. **Walk `method_definition` children inside the class body** — for each method, collect its decorators and look for `@Get`, `@Post`, `@Put`, `@Delete`, `@Patch`, `@Options`, `@Head`, `@All`.

3. **Compose full path** — combine `@Controller('prefix')` + `@Method('subpath')`:
   - `@Controller('articles')` + `@Get(':slug/comments')` → `/articles/:slug/comments`
   - `@Controller('articles')` + `@Get()` → `/articles`
   - `@Controller()` + `@Post('users')` → `/users`

4. **Auth detection via `@UseGuards`** — the argument text (e.g., `"AuthGuard('jwt')"`) already matches `patterns::AUTH_INDICATORS` (contains "auth", "guard", "jwt"). Use `AuthKind::Decorator(text)`.

5. **New helpers needed:**
   - `collect_decorators(node) -> Vec<(String, Option<String>)>` — name + first string arg
   - `is_nestjs_route_decorator(name) -> Option<HttpMethod>` — maps "Get"→GET, etc.

### Reference Implementation

The Java extractor's `collect_annotations` / `try_parse_mapping_annotation` pattern (lines 267-384 of `java.rs`) is a structural template — NestJS decorators are syntactically analogous to Spring Boot annotations.

### Additional NestJS Patterns (lower priority)

| Pattern | Complexity |
|---|---|
| Class-level `@UseGuards()` applying to all methods | Low — propagate auth to all routes in class |
| `@ApiBearerAuth()` Swagger decorator as auth signal | Low — add to AUTH_INDICATORS |
| `@Roles('admin')` custom decorator | Medium — similar to `@UseGuards` |
| Versioned controllers: `@Controller({ path: 'articles', version: '1' })` | Medium — parse object arg |

---

## GAP-02: Route Group Prefix Resolution (Go, PHP, C#)

**Severity:** Medium
**Impact:** Endpoints detected but with empty or relative path strings, reducing the value of endpoint queries.
**Files:** `go.rs`, `php.rs`, `csharp.rs`

### Problem

Multiple frameworks use a "group" pattern where the full URL is composed from a prefix defined elsewhere + a relative path on the route registration. The extractor only sees the local file and extracts the relative path.

### Go (Gin RouterGroup)

```go
// hello.go — defines the group prefix
v1 := r.Group("/api")
users.UsersRegister(v1.Group("/users"))

// users/routers.go — registers routes with relative paths
func UsersRegister(router *gin.RouterGroup) {
    router.POST("", UsersRegistration)     // extracted as path: ""
    router.POST("/login", UsersLogin)      // extracted as path: "/login"
}
```

**Full URL** should be `/api/users` and `/api/users/login`, but we extract `""` and `"/login"`.

**Root cause:** `r.Group("/api")` returns a `*RouterGroup` assigned to a variable. Tracking this requires cross-file data flow analysis — following variable assignments across function boundaries.

### PHP (Laravel Route Groups)

```php
Route::middleware(['auth'])->group(function () {
    Route::get('/dashboard', [DashController::class, 'index']);
});

Route::prefix('api/v1')->middleware('auth:sanctum')->group(function () {
    Route::get('/users', [UserController::class, 'list']);
});
```

**Inner `Route::get('/dashboard')` is detected**, but without the `->middleware(['auth'])` context. The extractor checks for middleware chaining only immediately after `Route::method()`, not on group-level `Route::middleware()`.

**`Route::prefix('api/v1')` is invisible** — inner routes get their local path without the prefix.

### C# (ASP.NET Minimal API MapGroup)

```csharp
var api = app.MapGroup("/api/v1");
api.MapGet("/items", handler);          // extracted as path: ""
api.MapPost("/items", handler);         // extracted as path: ""
```

**Root cause:** Same as Go — `MapGroup()` returns a `RouteGroupBuilder` assigned to a variable. The subsequent `api.MapGet()` calls are `invocation_expression` nodes, but the extractor doesn't recognize `MapGet` as a route method.

### C# (Controller-Level `[Route]` Prefix)

```csharp
[Route("api/[controller]")]
[ApiController]
public class CatalogController : ControllerBase {
    [HttpGet]            // Should resolve to /api/catalog — currently empty
    public IActionResult List() { ... }

    [HttpGet("{id}")]    // Should resolve to /api/catalog/{id} — currently just "{id}"
    public IActionResult Get(int id) { ... }
}
```

The extractor handles `[HttpGet("path")]` on methods but does NOT compose class-level `[Route("prefix")]` with method-level paths.

### Proposed Solution: Two-Phase Approach

**Phase 1 (Quick Win) — Per-file group context propagation:**

For each framework, track a "current prefix" stack during the CST walk. When entering a `Route::prefix('x')->group(closure)`, push `x` onto the prefix stack. When a `Route::get('/path')` is found inside, prepend the current prefix. Pop on closure exit.

This works for single-file route definitions (covers ~60% of real-world Laravel and C# controller apps).

**Phase 2 (Cross-file) — TwinBuilder post-processing:**

After all files are extracted, use `references` and `imports` to resolve group assignments:
1. Find `r.Group("/api")` calls and track what variable they're assigned to
2. Find functions that receive the group variable as a parameter
3. Prepend the group prefix to all routes registered within that function

This is architecturally similar to `import_resolver.rs` — it's a cross-file linking pass.

---

## GAP-03: ASP.NET Minimal API Endpoints (14 detected, paths empty)

**Severity:** High
**Impact:** ASP.NET Minimal APIs are the default for new .NET projects since .NET 6 (2021). The eShop reference architecture (Microsoft's official sample) uses them exclusively.
**File:** `crates/intently_core/src/twin/extractors/csharp.rs`

### Problem

The C# extractor only handles traditional controller-based `[HttpGet]` attribute routing. ASP.NET Minimal APIs use a completely different pattern:

```csharp
// Minimal API — NOT detected
app.MapGet("/api/catalog/items", (CatalogServices services) => { ... });
app.MapPost("/api/catalog/items", handler);
app.MapPut("/api/catalog/items/{id}", handler);
app.MapDelete("/api/catalog/items/{id}", handler);
```

These are `invocation_expression` nodes in the CST. The extractor visits them but only checks for `HttpClient` calls and log sinks — there is no code to match `MapGet`, `MapPost`, etc.

### Missing Patterns

| Pattern | CST Node | Priority |
|---|---|---|
| `app.MapGet("/path", handler)` | `invocation_expression` → `member_access_expression("MapGet")` | **P0** |
| `app.MapPost("/path", handler)` | same, method name `"MapPost"` | **P0** |
| `app.MapPut("/path", handler)` | same, method name `"MapPut"` | **P0** |
| `app.MapDelete("/path", handler)` | same, method name `"MapDelete"` | **P0** |
| `app.MapPatch("/path", handler)` | same, method name `"MapPatch"` | **P0** |
| `.RequireAuthorization()` chained | `invocation_expression` parent chain | **P1** |
| `app.MapGroup("/prefix")` | `invocation_expression`, variable assignment tracking | **P2** |
| `[Authorize]` on lambda parameters | attribute on parameter, not method | **P2** |

### Required Changes

1. **Add Minimal API method detection to `invocation_expression` handler** (line 55 of `csharp.rs`):

   When an `invocation_expression` has a `member_access_expression` child whose method name starts with `Map` and matches a known verb:
   - `MapGet` → GET, `MapPost` → POST, `MapPut` → PUT, `MapPatch` → PATCH, `MapDelete` → DELETE

2. **Extract path from first argument** — the first string literal in the `argument_list` is the route path.

3. **Detect `.RequireAuthorization()` chaining** — check if the `invocation_expression` is itself the receiver of a chained call to `RequireAuthorization`. Walk up the CST parent to find `member_access_expression` with identifier `RequireAuthorization`. Store as `AuthKind::Attribute("RequireAuthorization")`.

4. **Controller-level `[Route]` prefix composition** — when processing a `class_declaration` with `[Route("api/[controller]")]`, store the prefix. When processing methods inside that class, prepend the prefix. Replace `[controller]` with the lowercase class name minus "Controller" suffix.

### Existing Methods to Map

```
MapGet    → HttpMethod::Get
MapPost   → HttpMethod::Post
MapPut    → HttpMethod::Put
MapPatch  → HttpMethod::Patch
MapDelete → HttpMethod::Delete
MapMethods → HttpMethod::All (with specific method list)
```

---

## GAP-04: Symbol Extraction for Java and C# (0 and 2 symbols)

**Severity:** Medium
**Impact:** Without symbols, the MCP `query_symbols` tool returns empty results for Java/C# projects, degrading the knowledge graph.
**File:** `crates/intently_core/src/twin/extractors/symbols.rs`

### Investigation Results

The symbol queries for Java (`JAVA_SYMBOLS_QUERY`) and C# (`CS_SYMBOLS_QUERY`) ARE correctly defined and pass unit tests. The likely root cause for zero/near-zero symbols in real-world validation is **tree-sitter grammar version incompatibility with modern language features**.

### Java — Spring PetClinic (0 symbols from 47 files)

The query matches: `class_declaration`, `method_declaration`, `interface_declaration`, `enum_declaration`.

**Possible causes:**
1. `tree-sitter-java = "0.23"` may not parse all Java 17+ features used by Spring PetClinic (records, sealed classes, text blocks, pattern matching in switch)
2. If tree-sitter produces error nodes instead of valid AST, queries find no matches
3. Query compilation failure is silently swallowed (logged as `warn!` but returns empty vec)

**Diagnostic action:** Run `intently-mcp` with `RUST_LOG=warn` and check for `"failed to compile symbol query"` warnings.

### C# — eShop (2 symbols from 538 files)

The 2 symbols are `scrollToEnd` and `submitOnEnter` — both JavaScript functions from `src/ClientApp/`. Zero symbols from the 400+ C# files.

**Possible causes:**
1. `tree-sitter-c-sharp = "0.23"` may not support C# 12/13 features: primary constructors (`class Service(ILogger logger)`), collection expressions, `required` modifier, raw string literals
2. Modern C# uses file-scoped namespaces (`namespace Foo;` without braces) — if the grammar doesn't handle these, parsing fails for the entire file
3. `record` declarations are NOT in `CS_SYMBOLS_QUERY` — all record types are invisible

**Missing node types in C# symbol query:**
| Node Type | Description | Priority |
|---|---|---|
| `record_declaration` | C# 9+ record types (widely used in .NET 8+) | **P0** |
| `constructor_declaration` | Constructors | P1 |
| `property_declaration` | Public properties (API surface) | P2 |

### Recommended Actions

1. **Add diagnostic logging** — when `extract_symbols` returns empty for a file that has `>10` lines, log a warning with the file path and language

2. **Add `record_declaration` to `CS_SYMBOLS_QUERY`:**
   ```
   (record_declaration name: (identifier) @name) @definition.class
   ```

3. **Verify grammar compatibility** — parse a sample Spring PetClinic / eShop file with `tree-sitter-java 0.23` / `tree-sitter-c-sharp 0.23` and check for error nodes:
   ```rust
   let tree = parser.parse(source, None)?;
   let root = tree.root_node();
   if root.has_error() {
       warn!("Parse errors in {}: tree-sitter grammar may be outdated", file_path);
   }
   ```

4. **Consider grammar version upgrades** — check if newer versions of `tree-sitter-java` and `tree-sitter-c-sharp` are compatible with `tree-sitter = "0.25"`

---

## GAP-05: Laravel 11 Routing Patterns

**Severity:** Low (Laravel 11 skeleton has only 1 route; real apps still use `Route::method()`)
**Impact:** Resource routes, route groups with middleware/prefix, and `Route::any/match` not detected.
**File:** `crates/intently_core/src/twin/extractors/php.rs`

### What Works

`Route::get('/path', handler)` and `Route::post('/path', handler)->middleware('auth')` are correctly detected. The basic Laravel routing works.

### What Doesn't Work

| Pattern | Status | Priority |
|---|---|---|
| `Route::resource('photos', PhotoController::class)` | Not detected — `resource` fails `parse_http_method()` | **P1** |
| `Route::apiResource('posts', PostController::class)` | Not detected — same reason | **P1** |
| `Route::any('/path', handler)` | Not detected — `any` not in HTTP methods | **P1** |
| `Route::match(['get','post'], '/path', handler)` | Not detected — `match` not in HTTP methods | **P2** |
| `Route::middleware(['auth'])->group(closure)` | Inner routes detected but without auth context | **P1** |
| `Route::prefix('api/v1')->group(closure)` | Inner routes detected but without prefix | **P1** |
| `Application::configure()->withRouting(health: '/up')` | Not detected — scope is `Application`, not `Route` | **P2** |

### Required Changes

1. **Add `Route::resource()` / `Route::apiResource()` expansion:**
   - `resource('photos', Controller)` → 7 routes: index (GET /photos), create (GET /photos/create), store (POST /photos), show (GET /photos/{photo}), edit (GET /photos/{photo}/edit), update (PUT/PATCH /photos/{photo}), destroy (DELETE /photos/{photo})
   - `apiResource('posts', Controller)` → 5 routes (no create/edit)

2. **Add `Route::any()` support** — map to `HttpMethod::All`

3. **Route group context** — see GAP-02 Phase 1 for the per-file approach with prefix/middleware stacks

---

## GAP-06: False Positive Health Scores

**Severity:** Low
**Impact:** Projects where extraction fails appear healthy (100% score) because no endpoints = no violations.
**File:** `crates/intently_core/src/health/` (or wherever health is computed)

### Problem

NestJS project: 0 endpoints detected → 0 policy violations → health score 100%. This is misleading — the project isn't healthy, our analysis is incomplete.

### Proposed Fix

Add a confidence/coverage indicator to the health score:

```rust
pub struct HealthReport {
    pub overall: f64,
    pub security: f64,
    pub reliability: f64,
    // NEW: extraction confidence
    pub confidence: f64,  // 0.0-1.0 based on extraction coverage
}
```

**Confidence heuristics:**
- If `total_interfaces == 0` but files contain known web framework imports → confidence = 0.0
- If `total_symbols == 0` but file count > 10 → confidence reduced
- Base confidence from `(files_with_extractions / total_files)` ratio

---

## Priority Matrix

| Gap | Severity | Effort | Impact | Priority |
|---|---|---|---|---|
| **GAP-01**: NestJS decorators | High | Medium | Unblocks #1 Node.js framework | **P0** |
| **GAP-03**: ASP.NET Minimal APIs | High | Medium | Unblocks official .NET pattern | **P0** |
| **GAP-04**: Java/C# symbol extraction | Medium | Low | Diagnostic + grammar check | **P0** |
| **GAP-02**: Route group prefix resolution | Medium | High | Cross-file data flow problem | **P1** |
| **GAP-05**: Laravel resource routes | Low | Low | Simple method name additions | **P1** |
| **GAP-06**: False positive health scores | Low | Low | UX improvement | **P2** |

---

## Decision

Document these gaps and address in priority order. GAP-01, GAP-03, and GAP-04 should be resolved before public release as they affect the most popular frameworks in their respective ecosystems.
