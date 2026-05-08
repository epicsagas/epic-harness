# Syntagma + Orbit Demo Prompt Guide

Prompts to type into Claude Code during recording, in order.
Wait for each response to fully complete before entering the next prompt.

---

## Phase 1: Code Analysis (Syntagma auto-trigger)

```
Analyze user_service.py for code quality issues. Use syntagma tools to detect smells, suggest refactorings, and explore the knowledge graph. When done, save the full analysis results to syntagma-analysis.md in the project root.
```

> _dispatch automatically routes to syntagma: analyze_code -> suggest_refactorings -> search_knowledge.
> Results are written to syntagma-analysis.md so /orbit can reference them.
> Wait for the full response before proceeding.

---

## Phase 2: Knowledge Graph Deep Dive (optional, adds depth)

```
Explore the knowledge graph around the detected smells. Use get_neighbors and find_path to show how SMELL-01 connects to design patterns and refactoring techniques.
```

> Wait for the graph traversal output.

---

## Phase 3: /orbit

```
/epic:orbit
```

> Orbit detects syntagma results in context and auto-enters Direct Mode.
> The full pipeline runs: Spec -> Go -> Check -> Ship -> Evolve.
> This is the longest phase (~3-5 min).

---

## Phase 4: Post-flight (optional)

```
/evolve history
```

> Shows session analysis and evolved skill attribution.

---

## Notes

- Wait for each response to complete before typing the next prompt
- `/orbit` is the longest segment (typically 3-5 minutes)
- End recording: `/exit` in Claude Code, then `Ctrl+D` or `exit` for asciinema
- Convert to GIF: `make demo-gif`
