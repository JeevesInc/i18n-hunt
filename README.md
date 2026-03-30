# i18n-hunt

> Find unused i18n translation keys in JavaScript projects using AST analysis

---

## ✨ Why this exists

Managing i18n keys gets messy fast.

As your product evolves, translations change constantly — new keys are added, old ones become unused, and over time it becomes hard to know what is still in use.

**i18n-hunt helps answer a simple question:**

> _"Is this translation key still used in my codebase?"_

It scans your source code and locale files to highlight keys that are no longer referenced.

---

## 🚧 Status

**Experimental (WIP)**

The project is currently in an early stage and under active development.  
The goal is to validate the approach, gather feedback, and evolve it into a stable CLI.

---

## 🚀 Getting Started

For now, the CLI is not published yet.

Run it locally using Cargo:

```
cargo run -- \
  --locales "public/locales/en-US/" \
  --src "src/"
```

---

## ⚙️ Usage

Basic usage:

```
hunt --locales "public/locales/en-US" --src "src/"
```

### Parameters

- `--locales` → Root directory of your locale JSON files
- `--src` → Source code directory to scan (JS/TS/JSX/TSX)

---

## 📦 Examples

```
# Scan entire project
hunt --locales "public/locales/en" --src "src/"
```

> Planned (WIP):

```
# Scan a specific locale folder
hunt --locales "public/locales/en/TeamManagement" --src "src/"

# Context-aware scan (more focused + faster)
hunt --locales "public/locales/en/TripRequest" --src "src/views/trip-request/"
```

---

## 🧠 How it works

i18n-hunt analyzes your code using [AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree).

It classifies usages into:

- **Static keys** → directly detected (`t("form.email")`)
- **Prefixes** → partially dynamic but still safe (`t(`form.${field}`)`)
- **Dynamic usage** → tracked but not aggressively marked as unused

This approach avoids false positives while still surfacing real unused keys.

---

## 📤 Output

Example:

```
[Auth/Login] -> legacy.oldLoginMessage
```

Each result shows:

- the namespace (based on file structure)
- the unused key

---

## 🗺️ Roadmap

Planned improvements (subject to change):

- Better handling of dynamic keys
- Package manager wrapper (run via `npm`, `pnpm`, `yarn` / integrate with CI)
- Improved output formatting (DX)
- Config file (`i18n-hunt.config`)
- Scoped scans:
  - specific JSON files
  - specific source directories/files
- Auto-remove unused keys
- Respect `.gitignore` (under evaluation)

---

## 🤝 Contributing

Contributions are welcome — especially at this stage.

Good ways to contribute:

- Share real-world edge cases (very valuable)
- Report false positives / false negatives
- Suggest improvements for CLI UX
- Help shape the config and workflow

If you're using i18n-hunt in a real project, your feedback is gold.

---

## 💡 Notes

- Works with any JavaScript/TypeScript project
- Designed to be safe-first (avoids aggressive deletion)
- Built with Rust for performance and reliability
