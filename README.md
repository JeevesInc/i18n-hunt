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
- `--config` → Optional TOML config file (defaults to `i18n-hunt.toml` when present)

### Config (`i18n-hunt.toml`)

```toml
locales = "./locales"
src = "./src"

# Optional: skip paths
src_exclude = ["**/*.test.ts", "legacy/**"]
locales_exclude = ["Legacy/**"]
```

Both `locales` and `src` can point to either a directory or a specific file.

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
Unused translation keys:

[Auth/Login] public/locales/en/Auth/Login.json -> legacy.oldLoginMessage
[Common] public/locales/en/Common.json -> legacy.oldTransLabel

Total unused keys: 2

Dynamic translation usage sites:

src/pages/login.ts:42 -> [Auth/Login]
src/pages/locations.ts:31 -> [TeamManagement/Locations]

Total dynamic usages: 2
```

Unused key entries show:

- the namespace (based on file structure)
- the locale file path
- the unused key

Dynamic usage entries show:

- source file and line
- namespaces in scope when the unresolved key was found

If nothing is found, the CLI prints:

```
No unused translation keys found.
```

---

## 🗺️ Roadmap

Planned improvements (subject to change):

- Package manager wrapper (run via `npm`, `pnpm`, `yarn` / integrate with CI)
- Auto-remove unused keys (safe mode first: dry-run + high-confidence only)

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
