# i18n-hunt

> Find unused i18n translation keys in JavaScript projects using AST analysis.

## ✨ Why this exists

Managing i18n keys gets messy fast.

As applications evolve, translation files tend to accumulate stale keys that are no longer referenced anywhere in the codebase.

**i18n-hunt** helps identify those unused translations before they become noise.

## 🚧 Status

**Experimental (WIP)**

The project is currently in an early stage and under active development.

The goal is to validate the approach, gather feedback, and evolve it into a stable CLI.

## 🚀 Installation

```bash
npm install @jeevesinc/i18n-hunt -D
```

### ⚙️ Usage

Create an `i18n-hunt.toml` file:

```toml
locales = "./public/locales/en-US"
src = "./src"

# Optional
src_exclude = ["**/*.test.ts", "legacy/**"]
locales_exclude = ["Legacy/**"]
```

Add a script to your `package.json`:

```json
{
  "scripts": {
    "hunt": "i18n-hunt"
  }
}
```

Run the scan:

```bash
npm run hunt
```

You can also run it directly:

```bash
i18n-hunt --locales "public/locales/en-US" --src "src/"
```

## 🧠 Detection strategy

i18n-hunt analyzes your source code using [AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree) analysis.

It classifies translation usage into:

* **Static keys** → directly detected (`t("form.email")`)
* **Prefixes** → partially dynamic but still safe (`t(`form.${field}`)`)
* **Dynamic usage** → tracked without aggressively marking keys as unused

This helps reduce false positives while still surfacing genuinely unused translations.

For supported patterns and detection details, see the [Wiki](https://github.com/JeevesInc/i18n-hunt/wiki).

## 🤝 Contributing

Contributions and feedback are welcome — especially at this stage.

Useful contributions include:

* Reporting false positives or false negatives
* Sharing real-world edge cases
* Improving CLI UX
* Suggesting workflow or config improvements

