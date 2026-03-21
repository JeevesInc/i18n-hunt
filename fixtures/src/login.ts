import { useTranslation } from "react-i18next";

export function LoginPage() {
    const { t } = useTranslation(["Auth/Login"]);

    // Static usages
    t("title");
    t("form.email");
    t("form.password");

    // Template prefix usage
    const field = "submit";
    t(`form.${field}`);
    t(`form.field.${action}`);

    // Constant variable (future inference)
    const errorKey = "errors.invalidCredentials";
    t(errorKey);

    // Dynamic usage
    const dynamicKey = buildErrorKey();
    t(dynamicKey);

    return null;
}

function buildErrorKey() {
    return Math.random() > 0.5
        ? "errors.network"
        : "errors.invalidCredentials";
}
