---
name: "#005 Benchmark current performance baselines"
about: "P0 Task - Establish perf metrics for comparison"
title: "[P0] #005 Benchmark current perf baselines (startup time, scan speed, memory)"
labels: "priority-0, yc-critical, performance, benchmarking"
assignees: ""
---

## 🎯 Parent Epic
#001 [EPIC] Tech Stack Upgrade: Strategy & Non-Goals

## 📋 Task Description

Establish comprehensive performance baselines for the current stack before any upgrades begin. These benchmarks will be used to verify that upgrades improve (or at least don't degrade) performance.

### Metrics to Capture

#### Startup Performance
- [ ] Cold start time (first launch)
- [ ] Warm start time (subsequent launches)
- [ ] Time to interactive UI
- [ ] Initial memory footprint

#### Organizer Scan Performance
- [ ] Scan speed for small workflows (<10 steps)
- [ ] Scan speed for medium workflows (10-50 steps)
- [ ] Scan speed for large workflows (>50 steps)
- [ ] Memory usage during scans
- [ ] CPU utilization during scans

#### Storage Performance
- [ ] Read latency for audit logs
- [ ] Write latency for new actions
- [ ] Database size growth rate
- [ ] Query performance for common operations

#### Execution Performance
- [ ] Time from approval to execution start
- [ ] Execution throughput (actions/second)
- [ ] Undo operation latency

## ✅ Acceptance Criteria

- [ ] Benchmark suite created at `/benchmarks/`
- [ ] Baseline results documented at `/docs/PERF_BASELINES.md`
- [ ] Automated benchmark CI job (runs on main branch)
- [ ] Performance regression thresholds defined (e.g., no >10% degradation)

## 🔗 Related Issues
- Parent: #001
- Related: #013 (tokio tuning), #015 (storage profiling)

## ⏱️ Effort Estimate
**Time:** 1 day  
**Complexity:** Medium  
**Risk:** Low (measurement only)

## 📝 Notes
Run benchmarks on multiple platforms (macOS ARM, macOS Intel, Windows) to capture platform-specific baselines.
