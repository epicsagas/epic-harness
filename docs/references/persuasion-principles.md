# Persuasion Psychology in Skill Design

Systematic application of persuasion principles to AI agent skill design, based on Cialdini (2021) and Meincke et al. (2025, N=28,000).

## Principles Used

### Authority
- Source: Domain expertise and research backing
- Application: Iron Laws cite research. Skills reference benchmark data.
- Example: "NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST" — backed by TDD research showing 40-80% defect reduction.

### Commitment & Consistency
- Source: Agents commit to a process before starting
- Application: TDD's RED-GREEN-REFACTOR cycle forces sequential commitment
- Example: Writing a failing test first creates commitment to making it pass

### Social Proof
- Source: Adoption statistics and benchmark results
- Application: Reference project analysis results. 9/9 reference projects used similar patterns.
- Example: "MCP-Atlas 79.4% (world #1), SWE-bench 76.8%" — a-evolve benchmark results

## Principles Intentionally Excluded

### Liking (EXCLUDED)
- Why excluded: Liking-based persuasion (flattery, similarity) exploits emotional bias
- Risk in AI: Agent might agree with poor decisions to be "likable"
- Replacement: Objective evidence-based feedback, even when contradictory

### Reciprocity (EXCLUDED)
- Why excluded: Reciprocity (favors create obligation) is manipulative in AI context
- Risk in AI: Agent might feel obligated to accept user's bad approach after providing help
- Replacement: Clear boundary between agent assistance and user decision-making

## Iron Law → Persuasion Principle Mapping

| Iron Law | Location | Principle | Rationale |
|----------|----------|-----------|-----------|
| NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST | tdd/SKILL.md | Authority + Commitment | Authority: TDD research. Commitment: test-first locks in the requirement. |
| NO DEPLOY WITHOUT SECURITY VERIFICATION FIRST | secure/SKILL.md | Authority + Social Proof | Authority: OWASP standards. Social Proof: breach statistics. |
| NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST | debug/SKILL.md | Commitment + Authority | Commitment: investigate before fixing. Authority: systematic debugging literature. |

## Design Guidelines

1. **Every Iron Law must map to at least one persuasion principle** — unanchored rules are weaker
2. **Never use Liking or Reciprocity** — these create emotional manipulation risk in AI agents
3. **Authority requires genuine backing** — don't fabricate citations; use real research or remove the authority claim
4. **Social Proof must be specific** — "many people do this" is weak; "79.4% on MCP-Atlas" is strong
5. **Commitment works best with sequential gates** — RED before GREEN before REFACTOR

## References

- Cialdini, R. B. (2021). Influence, New and Expanded: The Psychology of Persuasion
- Meincke, A., et al. (2025). "Persuasion in AI-Agent Interactions" (N=28,000)
- Superpowers (obra/Prime Radiant) — first framework to systematically apply these principles to AI skill design
