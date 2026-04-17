import { useTranslation } from "react-i18next";

export function FallbackPage() {
    const { t } = useTranslation(["Auth/Login", "Common"]);

    t("sharedFallback");
    t("onlyInCommon");

    return null;
}
