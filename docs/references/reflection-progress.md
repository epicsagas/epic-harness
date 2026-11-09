# Reflection progress and merged histories

Reflection consumes observations using a project-scoped database row cursor.
Its evolution record, metrics, and updated cursor commit together. If that save
fails, the observations remain eligible for a later reflection. Equal timestamps
and delayed arrivals are separate observations rather than duplicate delivery.

For an existing timestamp watermark, the first cursor establishes the boundary
of already analyzed database rows before importing delayed JSONL fallback data.
The stored observations and existing metrics remain intact.

Copying a project database allocates destination row IDs, and relabeling project
histories combines independent progress boundaries. These operations reset the
derived reflection cursor to zero instead of trying to reuse either cutoff.
This also initializes progress for legacy histories that only had a timestamp
watermark, so newly imported older records remain eligible for analysis.
Observations remain stored; the next reflection may analyze existing history
again. This conservative replay avoids silently skipping imported or previously
pending observations.
