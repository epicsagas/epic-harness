# Agent Team Architecture Patterns

## 1. Pipeline
```
Agent A → Agent B → Agent C
```
- **When**: Sequential dependent tasks (analyze → implement → test)
- **Strength**: Clear data flow, easy to debug
- **Weakness**: No parallelism, bottleneck at slowest stage

## 2. Fan-out / Fan-in
```
        ┌→ Agent A ─┐
Input ──┼→ Agent B ──┼→ Merge → Output
        └→ Agent C ─┘
```
- **When**: Independent parallel tasks (review + test + lint)
- **Strength**: Maximum parallelism
- **Weakness**: Merge step can be complex

## 3. Expert Pool
```
Router → Agent A (if domain=auth)
       → Agent B (if domain=frontend)
       → Agent C (if domain=data)
```
- **When**: Tasks vary by domain, need specialized knowledge
- **Strength**: Deep expertise per domain
- **Weakness**: Router logic can be fragile

## 4. Producer-Reviewer
```
Producer → Artifact → Reviewer → Approved / Rejected
                                    ↓ (if rejected)
                                 Producer (retry)
```
- **When**: Quality is critical, need adversarial check
- **Strength**: Catches errors before they propagate
- **Weakness**: Can loop if quality bar is unclear

## 5. Supervisor
```
Supervisor ──┬── Worker A
             ├── Worker B
             └── Worker C
             (dynamic assignment based on progress)
```
- **When**: Dynamic workload, tasks may spawn sub-tasks
- **Strength**: Adaptive, handles surprises
- **Weakness**: Supervisor is a single point of failure

## 6. Hierarchical Delegation
```
Lead ── Sub-lead A ── Worker A1
    │              └── Worker A2
    └── Sub-lead B ── Worker B1
```
- **When**: Large scope, needs recursive decomposition
- **Strength**: Scales to complex projects
- **Weakness**: Communication overhead, deep nesting

## Selection Guide

| Project Size | Recommended | Why |
|-------------|-------------|-----|
| Small (1-3 files) | Pipeline | Simple, predictable |
| Medium (feature) | Fan-out/Fan-in | Parallel review+test+implement |
| Cross-cutting | Expert Pool | Each domain handled by specialist |
| Quality-critical | Producer-Reviewer | Adversarial quality check |
| Large/dynamic | Supervisor | Adaptive to changing scope |
| Enterprise | Hierarchical | Scales with team decomposition |
