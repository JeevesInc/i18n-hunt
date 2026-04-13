import { useTranslation } from "react-i18next";

export function LocationsPage() {
    const { t } = useTranslation(["TeamManagement/Locations"]);

    t("title");
    t("table.name");
    t("Auth/Login:colonUsed");
    t("optionNsOnly", { ns: "Auth/Login" });

    const action = "create";
    t(`actions.${action}`);

    const notificationKey = getNotificationKey();
    t(notificationKey);

    return null;
}

function getNotificationKey() {
    return Math.random() > 0.5
        ? "notifications.created"
        : "notifications.deleted";
}
