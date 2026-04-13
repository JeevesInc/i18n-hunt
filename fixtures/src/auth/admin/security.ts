import { useTranslation } from "react-i18next";

export function SecurityPage() {
    const { t } = useTranslation(["Auth/Admin/Security"]);

    t("title");

    const action = "enable";
    t(`mfa.${action}`);

    return null;
}
