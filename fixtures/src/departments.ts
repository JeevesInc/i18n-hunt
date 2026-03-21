import { useTranslation } from "react-i18next";

export function LocationsPage() {
    const { t } = useTranslation(["TeamManagement/Departments"]);

    // Static usage
    t("title");
    t("table.name");
    t("table.address");

    // Template prefix usage
    const action = "create";
    t(`actions.${action}`);

    // Another prefix
    const status = "active";
    t(`status.${status}`);

    // Dynamic usage
    const key = getNotificationKey();
    t(key);

    return null;
}

function getNotificationKey() {
    return Math.random() > 0.5
        ? "notifications.created"
        : "notifications.deleted";
}
