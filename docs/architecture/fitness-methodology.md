# Architecture Fitness Methodology

*A Codegraph-Based Approach for Architecture Validation*

> "Architecture is about dependencies. Everything else follows from that."
> — Inspired by Robert C. Martin (Uncle Bob)

## Overview

This methodology provides automated, objective architecture validation using SCIP-based semantic code intelligence (codegraph). Instead of relying on intuition and code reviews, we measure architectural fitness through concrete metrics.

## The Five Fitness Functions

### FF-1: Dependency Direction Check

> "Dependencies must point inward, toward higher-level policies."

**Principle:** The Dependency Rule from Clean Architecture states that source code dependencies must point only inward, toward higher-level policies.

**What to measure:**
- Core domain modules should have LOW efferent coupling (few outgoing deps)
- Infrastructure modules should have HIGH afferent coupling (many depend on them)
- UI/Presentation should depend on domain, NEVER vice versa

**Codegraph implementation:**
```bash
# Check if domain code depends on infrastructure
get_module_deps("src/domain.rs")  # Should NOT include "src/ui.rs", "src/http.rs"
```

**Pass criteria:**
- Domain modules have zero dependencies on UI/infrastructure
- Dependency arrows point from outer layers to inner layers

---

### FF-2: Component Instability Metric

> I = Ce / (Ca + Ce)

Where:
- **Ce** = Efferent coupling (outgoing dependencies - what I depend on)
- **Ca** = Afferent coupling (incoming dependencies - what depends on me)
- **I** = Instability (0 = maximally stable, 1 = maximally unstable)

**Principle:** Stable components (I→0) should be depended upon. Unstable components (I→1) should depend on stable ones. Never have an unstable component depended upon by a stable one.

**Example measurement:**

| Module | Ce (out) | Ca (in) | I = Ce/(Ca+Ce) | Status |
|--------|----------|---------|----------------|--------|
| config.rs | 0 | 16 | 0.00 | Stable |
| history.rs | 2 | 8 | 0.20 | Stable |
| ui.rs | 19 | 2 | 0.90 | Unstable |

**Pass criteria:**
- Stable components (I < 0.3) are depended upon by unstable ones
- No stable component depends on an unstable component

---

### FF-3: Hotspot Risk Analysis

> "High-caller symbols are architectural pressure points."

**Principle:** Symbols with many callers represent high-risk areas. Changes to these symbols ripple through the codebase. They should be:
1. Extremely stable (rarely changed)
2. Well-tested
3. Hidden behind abstractions

**Codegraph implementation:**
```bash
find_hotspot_symbols(min_callers=10)
```

**Risk thresholds:**

| Callers | Risk Level | Action Required |
|---------|------------|-----------------|
| 5-10 | Low | Monitor |
| 10-20 | Medium | Ensure test coverage |
| 20+ | High | Consider abstraction, must be stable |

**Pass criteria:**
- Hotspots with >20 callers have >80% test coverage
- Hotspots are in stable modules (I < 0.3)

---

### FF-4: Module Size / Cohesion

> "A class should have only one reason to change." — Single Responsibility Principle

**Principle:** Large modules with many symbols indicate low cohesion and multiple responsibilities. They should be split along responsibility boundaries.

**Codegraph implementation:**
```bash
get_file_symbols("src/ui.rs")  # Count symbols
```

**Thresholds:**

| Symbols | Assessment | Action |
|---------|------------|--------|
| < 100 | Good | None |
| 100-200 | Warning | Review responsibilities |
| > 200 | Violation | Must split |

**Pass criteria:**
- No module exceeds 200 symbols
- Each module has a single, clear responsibility

---

### FF-5: Cyclic Dependency Detection

> "There must be no cycles in the component dependency graph." — Acyclic Dependencies Principle

**Principle:** Cycles in the dependency graph make the system hard to understand, test, and deploy independently. They indicate missing abstractions.

**Detection method:**
1. Build dependency graph from `get_module_deps()` for all modules
2. Run cycle detection (DFS-based)
3. Flag any cycles found

**Resolution strategies:**
1. **Dependency Inversion:** Extract interface in the depended-upon module
2. **Extract Common:** Move shared code to a new module both depend on
3. **Merge:** If modules are truly cohesive, merge them

**Pass criteria:**
- Zero cycles in the module dependency graph

---

## Implementation Workflow

### Architecture Fitness Check Process

```
┌─────────────────────────────────────────────────────────┐
│                ARCHITECTURE FITNESS CHECK               │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. LOAD INDEX                                          │
│     load_project_indexes()                              │
│                                                         │
│  2. DEPENDENCY DIRECTION                                │
│     For each module:                                    │
│       - get_module_deps()                               │
│       - Check: core depends on infra? → FAIL            │
│                                                         │
│  3. INSTABILITY METRICS                                 │
│     For each module:                                    │
│       - Ca = count of files depending ON this           │
│       - Ce = count of files this DEPENDS on             │
│       - I = Ce / (Ca + Ce)                              │
│       - Flag: unstable depending on unstable            │
│                                                         │
│  4. HOTSPOT ANALYSIS                                    │
│     find_hotspot_symbols(min_callers=10)                │
│     Flag: hotspots not in "stable" modules              │
│                                                         │
│  5. SIZE/COHESION                                       │
│     For each file:                                      │
│       - get_file_symbols()                              │
│       - Flag: >200 symbols                              │
│                                                         │
│  6. CYCLE DETECTION                                     │
│     Build dependency graph                              │
│     Run cycle detection algorithm                       │
│     Flag: any cycles found                              │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Integration Points

### Before Every PR

Run architecture fitness checks to catch violations early:
```bash
claude "Run architecture fitness checks on this PR"
```

### During Planning

Before implementing a feature, analyze impact:
```bash
claude "Analyze impact of changing [SymbolName]"
```

This shows all call sites that would be affected by changes.

### For Refactoring

Identify extraction candidates:
```bash
claude "Find symbols in [file] with >5 callers that could be extracted"
```

---

## Codegraph Tool Reference

| Tool | Purpose | Use Case |
|------|---------|----------|
| `load_project_indexes` | Load SCIP index | Initial setup |
| `get_module_deps` | Get module dependencies | FF-1, FF-2, FF-5 |
| `find_hotspot_symbols` | Find high-caller symbols | FF-3 |
| `get_file_symbols` | List symbols in file | FF-4 |
| `get_callers` | Find all call sites | Impact analysis |
| `get_callees` | Find all calls from symbol | Dependency tracing |
| `get_impact` | Analyze change impact | Planning |

---

## Summary Checklist

| Fitness Function | Tool | Pass Criteria |
|-----------------|------|---------------|
| Dependency Direction | `get_module_deps` | Core has no infra deps |
| Instability | `get_module_deps` (both dirs) | Stable deps only |
| Hotspots | `find_hotspot_symbols` | <20 callers or tested |
| Cohesion | `get_file_symbols` | <200 symbols/file |
| Cycles | Derived from deps | Zero cycles |

---

## References

- Martin, R. C. (2017). *Clean Architecture: A Craftsman's Guide to Software Structure and Design*
- Martin, R. C. (2002). *Agile Software Development: Principles, Patterns, and Practices*
- SCIP (Source Code Intelligence Protocol) - https://sourcegraph.com/docs/code-intelligence/scip
