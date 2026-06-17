# Project Agents

Place project-specific agent definitions in this directory.

## Agent File Template

```markdown
---
description: Short description of what this agent does
mode: primary
color: "#49D1E3"
permission:
  edit:
    "src/**/*": "allow"
    "tests/**/*": "allow"
  bash:
    "cargo build*": "allow"
    "cargo test*": "allow"
    "cargo check*": "allow"
    "git status": "allow"
    "git diff*": "allow"
    "git commit*": "allow"
---

# Agent: agent-name-here

Your agent instructions here. Describe:
- What the agent does
- When to invoke it
- The exact workflow it follows
- Known pitfalls and anti-patterns
- How to interpret its output
```

## When to Create an Agent

- A workflow requires 5+ steps with specific conventions
- The workflow has known pitfalls that need explicit instructions
- Multiple slash commands will invoke the same workflow
- The workflow requires specific tool permissions
