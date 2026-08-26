# Agent-code Playground

See three bad habits that coding agents can leave behind even when a patch works: bypassing an existing boundary, copying a local workaround instead of the shared abstraction, and reusing a helper from the wrong side of the dependency graph.

Each scenario compares a real before/after repository fixture, connects the agent's local decision to the wider codebase, and presents the single Issue generated from the after state.

[Open the Playground](playground/)

The documentation build verifies that every before state has zero Issues and every after state has exactly one expected Issue. The Playground has no backend, uploads no code, and does not scan or evaluate third-party repositories.
