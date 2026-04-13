import { useTranslation } from "react-i18next";

export function LoginPage() {
    const { t } = useTranslation(["Auth/Login"]);

    t("title");
    t("form.email");
    t("form.password");

    const field = "submit";
    t(`form.${field}`);

    const dynamicKey = getErrorKey();
    t(dynamicKey);

    return null;
}

function getErrorKey() {
    return Math.random() > 0.5
        ? "errors.network"
        : "errors.invalidCredentials";
}
