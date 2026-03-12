import { robustFetch } from "../../lib/fetch";
import { fireEngineStagingURL, fireEngineURL } from "./scrape";
export async function fireEngineDelete(logger, jobId, mock, abort, production = true) {
    // jobId only supplied if we need to defer deletion
    if (!jobId) {
        logger.debug("Fire Engine job id not supplied, skipping delete");
        return;
    }
    await robustFetch({
        url: `${production ? fireEngineURL : fireEngineStagingURL}/scrape/${jobId}`,
        method: "DELETE",
        headers: {},
        logger: logger.child({ method: "fireEngineDelete/robustFetch", jobId }),
        mock,
        abort,
    });
    logger.debug("Deleted job from Fire Engine", { jobId });
}
//# sourceMappingURL=delete.js.map