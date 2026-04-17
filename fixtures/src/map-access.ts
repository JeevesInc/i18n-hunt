import { useTranslation } from "react-i18next";

export function MapAccessPage() {
    const { t } = useTranslation(["Auth/Login"]);

    const keyByState = {
        created: "mapCreated",
        deleted: "mapDeleted",
    };

    const state = "created";
    t(keyByState[state]);
    t(keyByState.deleted);

    return null;
}
