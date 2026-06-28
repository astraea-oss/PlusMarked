# Workspace Spec

A MarkdownPlus workspace is a portable local directory.

```text
workspace/
  workspace.toml
  notes/
  bases/
  .local/
    index.sqlite
```

`notes/` stores native `.mdp` files named by stable note UUID.

`.local/index.sqlite` is a rebuildable index. The `.mdp` files remain the durable source of note content.

