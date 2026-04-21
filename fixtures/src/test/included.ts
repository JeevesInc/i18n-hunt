import { useTranslation } from "react-i18next";

export function includedFixture() {
    const { t } = useTranslation("TestScope");
    t("includedOnly");
}
