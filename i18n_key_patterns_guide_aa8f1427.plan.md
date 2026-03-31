---
name: i18n key patterns guide
overview: A reference markdown document that explains common i18n usage patterns a static analyzer or CLI must recognize, grounded in typical react-i18next / i18next behavior and illustrated with real patterns from jeeves-web.
todos:
  - id: save-md
    content: "Optional: add docs/i18n-key-analysis-patterns.md in jeeves-web or CLI repo with this content"
    status: pending
  - id: cli-scope
    content: "Define CLI scope: include server + i18next.t, Trans, ns / Ns:key, dynamic key policy"
    status: pending
isProject: false
---

# i18n translation key patterns (for unused-key detection)

This document is meant to be saved as a `.md` file in your CLI project or docs. It explains **what your tool must parse or explicitly mark as “unknown”** so you do not falsely flag keys as unused or miss real usage.

---

## 1. Core API shapes

| Pattern                 | Example                                | Implications for tooling                                                                                                   |
| ----------------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `t('key')`              | `t('accountTitle')`                    | Straight string literal; easy to index.                                                                                    |
| Nested path             | `t('approvalRequested.description')`   | Same as flat key in i18next: path segments joined; JSON may be nested **or** a single key string containing dots (see §8). |
| `useTranslation(ns)`    | `useTranslation('Wallet')`             | Default namespace for `t` in that scope.                                                                                   |
| `useTranslation([...])` | `useTranslation(['Wallet', 'Common'])` | First namespace is default; fallback search order matters for resolution (not always trivial to mirror in a key indexer).  |

---

## 2. Namespace overrides (must resolve key + namespace together)

A key is only meaningful as `**(namespace, key)`, not the key alone.

- **Colon prefix:** `t('Filters:canceled')`, `t('Statuses:paid')`, `t('Accounting:someLabel')` — namespace explicit in the string ([example in repo](src/views/sat/filters-bar/filters.tsx)).
- **Option bag:** `t('error.invalidState', { ns: 'Vendor/Steps' })` — namespace in second argument ([example](src/views/vendor/index-page/utils.ts)); also `t(\`${type}.${key}, { ns: 'SAT' })` ([example](src/views/sat/helpers.tsx)).
- `**<Trans ns="Common" i18nKey="vendorOwnerTooltipV2" />` — `ns` prop on `Trans` ([example](src/views/vendor/components/vendor-owner/index.tsx)).

**CLI implication:** Build a map of `(file → active namespace(s))` from `useTranslation` / `Trans`, then merge with explicit `ns` and `Ns:key` strings.

---

## 3. `<Trans>` and `i18nKey`

- **Explicit:** `<Trans t={t} i18nKey="clickHereForDetails" />` — treat like `t('clickHereForDetails')` in the same namespace context ([examples](src/views/bulk-changes/index-page/index.tsx)).
- **Dynamic:** `<Trans i18nKey={key} t={t} />` — static analysis cannot know all keys without tracing `key` ([example](src/views/payments/components/invoice-automation-modal/index.tsx)).
- **Dynamic function:** `i18nKey={getDescriptionKey()}` — same limitation ([example](src/views/sat/connect-sat-landing-page/index.tsx)).

---

## 4. Dynamic / computed keys (high false-positive risk)

These patterns appear in real codebases and usually require **heuristics or allowlists**, not pure string matching.

| Pattern              | Example from this codebase                                                                                                                              |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Template literal     | `t(\`physicalCard.${cardTierKey})`,` t(deletePhysicalCardTitle.${cardData.cardTier})`                                                                   |
| Prefix + variable    | `t(\`${workflow.id}.title)` ([workflow-card.tsx](src/views/ai-agents/index-page/components/workflow-card.tsx))                                          |
| Key from map/array   | `t(RoutingCodeLabelMap[routingCode.type])`, `t(key)` in a map                                                                                           |
| Key from API         | `t(ensureString(data.message_key))` — keys may only exist at runtime ([use-ai-chat-streaming.ts](src/views/ai-insights/hooks/use-ai-chat-streaming.ts)) |
| Key from query param | `showErrorMessage(t(errorQueryParam))` ([login](src/views/login/index-page/index.tsx))                                                                  |

**CLI implication:** Report these as **“dynamic usage sites”**; optionally enumerate **suffix/prefix sets** from JSON (e.g. if JSON has `physicalCard.gold`, infer pattern `physicalCard.` only if you accept that level of guesswork).

---

## 5. Direct `i18next` / server translators

- **Global instance:** `i18next.t('error.missingEmail', options)` outside React ([vendor utils](src/views/vendor/index-page/utils.ts)).
- **Server:** `const t = await getServerTranslate('Accounting');` then `t('...')` in App Router pages ([e.g.](app/accounting-rules/setup/page.tsx)).

**CLI implication:** Scan `app/`, `src/`, and any shared packages; include `.ts` files, not only `.tsx`.

---

## 6. ICU message format in values (not extra keys)

Strings may contain `{count, plural, ...}` and `{x, select, ...}`. That does **not** create separate translation **keys**—it is one key per JSON entry. Do not treat ICU branches as missing keys.

Example entries: plural labels in locale JSON (see `public/locales/*/Filters.json`).

---

## 7. JSON file layout vs key names

- **Nested objects:** i18next flattens with dots: `{ "a": { "b": "x" } }` → key `a.b`.
- **Dots inside key names:** In this repo, keys like `"cashDescription.default"` are **literal single keys** with a dot in the name ([Wallet.json](public/locales/en-US/Wallet.json)), not necessarily nesting.

**CLI implication:** When loading JSON, flatten consistently with i18next’s rules, or compare using the same library.

---

## 8. Namespaces that are not simple file stems

Namespace can match paths: `getServerTranslate('JeevesPay/BulkPayments')` maps to `public/locales/<lang>/JeevesPay/BulkPayments.json`.

**CLI implication:** Namespace ↔ file path mapping must support subfolders.

---

## 9. Tests and duplicates

- Tests call `t('...')` too; include or exclude test folders consistently.
- Same key may be referenced in many files — your CLI should **dedupe “used keys”** before diffing against the JSON catalog.

---

## 10. Practical detection strategies (recommended)

1. **AST parsing** (TypeScript/`ts-morph` or Babel): find `CallExpression` to `t`, `i18next.t`, and JSX `Trans` with `i18nomeKey`.
2. **Track scope:** walk up to the nearest hook or infer default `ns` from `useTranslation` arguments; merge `Trans` `ns` and `t` second-arg `{ ns }`.
3. **Literal keys:** record full `Ns:key` or `(ns, key)`.
4. **Dynamic keys:** tag as unresolved; optionally expand maps/constants defined in the same file.
5. **Catalog:** load all locale JSONs for one language (e.g. `en-US`), flatten keys with correct nesting rules, exclude metadata keys if any.
6. **Output:** “unused keys”, “possibly used (dynamic)”, “keys in JSON missing from catalog”, etc.

---

## 11. What automated unused-key tools usually get wrong

- Ignoring **namespace**, so every collision looks like one key.
- Treating `**key_part1` + dynamic `part2` as unused when only the prefix is visible in code.
- Marking keys as unused when used only on **server** or in **non-React** modules.
- Forgetting **Crowdin-only** or **future** keys that are intentionally not wired yet (your product policy may still want them removed).

---

If you want this as a committed file in **jeeves-web**, say where it should live (e.g. `docs/i18n-key-analysis-patterns.md`); in **plan mode** no file was written. After you approve implementation, a follow-up step can add the file verbatim.
