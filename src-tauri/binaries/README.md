Sidecars are built locally by `scripts/build-sidecars.py` and named:

```
keysmith-claude-<target-triple>
keysmith-codex-<target-triple>
keysmith-grok-<target-triple>
keysmith-zcode-<target-triple>
```

They are not committed. Packaged builds copy them via Tauri `externalBin`.
Python is only a build-time dependency.
