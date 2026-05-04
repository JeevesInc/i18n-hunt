import { useTranslation } from "react-i18next";

const OPTIONS = ["title", "description"] as const;

export function IteratorKeysPage() {
    const { t } = useTranslation("Auth/Login");

    ["form.email", "form.password"].map((k) => t(k));
    OPTIONS.forEach((k) => t(k));

    return null;
}
