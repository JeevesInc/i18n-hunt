# i18n-hunt: Supported Patterns

This document describes what `i18n-hunt` currently detects when scanning source code and locale files.

## Scope

- Source file extensions: `.ts`, `.tsx`, `.js`, `.jsx`
- Locale file extension: `.json`
- Locale keys are flattened from nested JSON objects:
  - `{ "form": { "email": "..." } }` -> `form.email`

## Translation Call Patterns

### 1. `useTranslation(...)` + `t(...)`

Supported:

```ts
const { t } = useTranslation("Auth/Login");
t("title");
```

Also supports namespace arrays:

```ts
const { t } = useTranslation(["Auth/Login", "Common"]);
t("sharedFallback");
```

### 2. `i18next.t(...)`

Supported:

```ts
i18next.t("Auth/Login:i18nextOnly");
```

### 3. Namespace embedded in key (`ns:key`)

Supported:

```ts
t("Auth/Login:colonUsed");
```

### 4. Namespace override via options object

Supported:

```ts
t("optionNsOnly", { ns: "Auth/Login" });
```

### 5. `<Trans i18nKey="..." />`

Supported:

```tsx
<Trans i18nKey="transOnly" ns="Common" />
```

## Static / Prefix / Dynamic Key Resolution

`i18n-hunt` classifies key usages as:

- **Static**: exact key (e.g. `t("form.email")`)
- **Prefix**: stable prefix from template literal (e.g. `` t(`form.${field}`) `` -> `form.`)
- **Dynamic**: unresolved key expression

### Template literals

Supported:

```ts
t(`form.${field}`); // prefix: "form."
```

```ts
t(`title`); // static: "title"
```

Dynamic when no stable leading prefix:

```ts
t(`${section}.title`);
```

### Variable/const propagation

Supported for local `const` values:

```ts
const key = "title";
t(key);
```

### Conditional and logical fallbacks

Supported:

```ts
t(cond ? "a" : "b");
t(getMaybeKey() || "title");
t(getMaybeNullKey() ?? "form.submit");
```

Conservative behavior:

```ts
t(flag && "title"); // treated as dynamic overall
```

## Collections and Map-like Patterns

### Arrays

Supported:

```ts
t(["title", "description"]); // array elements analyzed
```

### Iterators (`map`, `forEach`)

Supported when callback first parameter is used as key:

```ts
["form.email", "form.password"].map((k) => t(k));
OPTIONS.forEach((k) => t(k));
```

### Object maps + computed/static access

Supported for `const` object maps:

```ts
const keyByState = { created: "mapCreated", deleted: "mapDeleted" };
t(keyByState[state]);   // computed member access
t(keyByState.deleted);  // static member access
```

## Function Return Inference

Supported for top-level function declarations returning keys:

```ts
function getErrorKey() {
  return Math.random() > 0.5 ? "errors.network" : "errors.invalidCredentials";
}
t(getErrorKey());
```

## Server Translator Pattern

Supported:

```ts
const t = await getServerTranslate("Accounting");
t("serverOnly");
```

## Exclude Patterns

Both source and locale scanning support glob excludes from config:

- `src_exclude`
- `locales_exclude`

Examples:

```toml
src_exclude = ["**/*.test.ts", "legacy/**"]
locales_exclude = ["Legacy/**"]
```

## Current Limitations

- Dynamic namespaces in `useTranslation(...)` are ignored.
- Imported cross-file constants/functions are not fully resolved (analysis is primarily intra-file).
- Only `.json` locale files are supported.
- Only JS/TS family source files are supported (`.js`, `.jsx`, `.ts`, `.tsx`).

## Output Semantics

- Unused keys are listed with key + locale file path.
- Dynamic/unresolved usages are reported with source file + line + namespaces in scope.
- Prefix usages protect any locale key that starts with the detected prefix.
