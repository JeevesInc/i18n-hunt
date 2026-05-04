import { useTranslation } from "react-i18next";

export function LogicalFallbackPage() {
    const { t } = useTranslation("Auth/Login");

    t(getMaybeKey() || "title");
    t(getMaybeNullKey() ?? "form.submit");

    return null;
}

function getMaybeKey() {
    return Math.random() > 0.5 ? "errors.network" : "";
}

function getMaybeNullKey() {
    return Math.random() > 0.5 ? null : "errors.invalidCredentials";
}
