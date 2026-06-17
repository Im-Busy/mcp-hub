# Project Commands

Place project-specific slash commands in this directory.

## Command File Template

```markdown
---
description: Short description shown in the command palette
---

# Command name displayed in the UI

Brief description of what this command does and when to use it.

## Usage

/command-name              # Basic usage
/command-name --flag value  # With options

## What It Does

| Step | Action |
|------|--------|
| 1.   | Step one description |
| 2.   | Step two description |
| 3.   | Step three description |
```

## When to Create a Command

- A workflow is invoked frequently enough to warrant a shortcut
- The workflow has specific arguments or options
- The workflow should be discoverable in the UI command palette
