---
description: Analyze the codebase and generate a high-fidelity 'Mission Prompt' for Codex/GPT-5.
---

1. **Strategic Audit (The Director)**
   - **Identify the Signal**: Determine which distinct parts of the codebase are *active* and *incomplete*. (e.g., "The mobile folder is empty", "The pairing logic has TODOs").
   - **Identify the Noise**: Determine which parts are *stable* and *irrelevant*. (e.g., "The core terminal emulation", "The verified backend protocols").
   - **Synthesize Directions**: Formulate 2-3 high-level "Vectors of Attack" or "Areas of Interest". These are leads, not instructions.

2. **Constructing the 'Strategic Director' Prompt**
   - Your goal is to orient Codex immediately without constraining its problem-solving.
   - **Structure the Prompt**:
     - **The Landscape**: "You are working on [Project Name]. It is a [High Level Description]."
     - **The 'No-Fly' Zone**: Explicitly list the huge chunks of code to **IGNORE**.
       - "Do not analyze `wezterm-core` or `deps`. They are stable."
       - "Refrain from refactoring the rust protocol unless necessary."
     - **The Frontier**: Point the camera at the areas needing work.
       - "Your focus is `lucidity-mobile`. It is currently a skeleton."
       - "The goal is to bridge the gap between [Component A] and [Component B]."
     - **The Challenge**:
       - "Explore the frontier. Investigate the missing links."
       - "Formulate your own plan to achieve [Goal]."
       - "Execute with high agency."

3. **Output**
   - Present the prompt in a distinct code block titled `## Strategic Codex Mission Prompt`.
   - Add a brief note on what you filtered out to save it time.

3. **Output**
   - Present the prompt in a distinct code block titled `## Optimized Codex Mission Prompt`.
   - Briefly explain what context you pre-loaded and why (e.g., "I've pre-loaded the `frame.rs` struct definitions so you don't have to look them up").
