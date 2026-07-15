---
issue_id: 031
parent_epic: 030
priority: P1
status: ⚪ Todo
labels: [frontend, typescript, architecture]
---

# #031 Evaluate Frontend Framework: Svelte vs. SolidJS vs. React

## 📋 Summary
Research and select a modern TypeScript-compatible frontend framework to replace vanilla JS while preserving Ghost's lightweight ethos.

## 🎯 Why This Matters
- **Maintainability**: Typed components reduce bugs and speed up development
- **Team velocity**: Better DX with HMR, component model, dev tools
- **YC demo**: Shows professional, modern stack
- **Future hiring**: Easier to onboard devs familiar with modern frameworks

## ✅ Acceptance Criteria
- [ ] Evaluation matrix created (bundle size, perf, DX, ecosystem)
- [ ] Proof-of-concept built in top 2 candidates
- [ ] Decision documented with rationale
- [ ] Migration path outlined for incremental adoption
- [ ] "No bundler" dev experience preserved OR vite setup validated

## 🔗 Related Issues
- Parent Epic: #030 (Frontend: Maintainable, Typed, Testable UI)
- Related: #032 (TypeScript migration), #037 (build experience)

## 🛠️ Implementation Notes
### Evaluation Criteria

| Criterion | Weight | Notes |
|-----------|--------|-------|
| Bundle size | High | Must stay lightweight (<100KB gzipped ideal) |
| Runtime perf | High | Organizer view renders many items |
| Learning curve | Medium | Team already knows JS well |
| Ecosystem | Medium | Component libraries, testing tools |
| TypeScript | High | First-class support required |
| Build tooling | High | Fast HMR, simple config |

### Candidates

**Svelte/SvelteKit**
- ✅ Smallest bundles, no virtual DOM
- ✅ Excellent TypeScript support
- ✅ Simple mental model
- ❌ Smaller ecosystem than React

**SolidJS**
- ✅ Best benchmarks, fine-grained reactivity
- ✅ JSX + TypeScript
- ✅ Small bundle
- ❌ Newer, smaller community

**React + Vite**
- ✅ Largest ecosystem
- ✅ Tons of component libraries
- ❌ Larger bundle, more complex
- ❌ Virtual DOM overhead

### Recommendation Lean
Leaning toward **Svelte** for:
- Closest to Ghost's "simple, fast, local-first" values
- Smallest footprint
- Easiest incremental migration (can mix with vanilla JS)

## 🧪 Testing Plan
- [ ] Build Organizer view component in each framework
- [ ] Measure bundle size with `vite build`
- [ ] Profile render performance with 1000+ items
- [ ] Test HMR speed during development

## ⏱️ Estimated Effort
**2-3 days** (research + PoC)

## 📝 Definition of Done
- [ ] Evaluation doc complete
- [ ] PoC branches for top candidates
- [ ] Team decision made
- [ ] Migration issue created for chosen framework

## 📊 Progress
- [ ] Research phase
- [ ] Build PoCs
- [ ] Benchmark
- [ ] Decision
- [ ] Document
