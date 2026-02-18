// This file is an exception to the "no supabase in scrapeURL" rule,
// and it makes me sad. - mogery
import { supabase_service } from "../../../services/supabase";
import { config } from "../../../config";
export function uploadScreenshot(meta, document) {
    if (config.USE_DB_AUTHENTICATION &&
        document.screenshot !== undefined &&
        document.screenshot.startsWith("data:")) {
        meta.logger.debug("Uploading screenshot to Supabase...");
        const fileName = `screenshot-${crypto.randomUUID()}.png`;
        supabase_service.storage
            .from("media")
            .upload(fileName, Buffer.from(document.screenshot.split(",")[1], "base64"), {
            cacheControl: "3600",
            upsert: false,
            contentType: document.screenshot.split(":")[1].split(";")[0],
        });
        document.screenshot = `https://service.firecrawl.dev/storage/v1/object/public/media/${encodeURIComponent(fileName)}`;
    }
    return document;
}
//# sourceMappingURL=uploadScreenshot.js.map