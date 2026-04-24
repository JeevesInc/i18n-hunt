import { useTranslation } from "react-i18next";

export function ignoredFixture() {
    const { t } = useTranslation("TestScope");
    t("excludedOnly");
}
