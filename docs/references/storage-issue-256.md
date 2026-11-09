# Retrying consolidation

An atomic source-row import ledger prevents repeated session, evolution and score-history imports while retaining distinct source rows with identical content. Writable sources receive a stable UUID, so moving their database preserves identity. Read-only sources use their canonical file location and need no metadata write; retain their location when retrying. This builds on the complete schema-aware transfer in #185.
