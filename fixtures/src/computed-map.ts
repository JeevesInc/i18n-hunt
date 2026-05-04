import { useTranslation } from "react-i18next";

export function ComputedMapPage() {
    const { t } = useTranslation("Auth/Login");

    const LoginStep = {
        GOOGLE_AUTH: "googleAuth",
        MAGIC_LINK: "magicLink",
    } as const;

    const TITLE = {
        [LoginStep.GOOGLE_AUTH]: "mapCreated",
        [LoginStep.MAGIC_LINK]: "mapDeleted",
    } as const;

    const pageType = getPageType();
    t(TITLE[pageType]);

    return null;
}

function getPageType() {
    return Math.random() > 0.5 ? "googleAuth" : "magicLink";
}
