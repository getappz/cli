import * as fs from "fs/promises";
import { config } from "../../../config";
import * as path from "path";
import { logger as _logger } from "../../../lib/logger";
const saveMocksDirPath = path.join(__dirname, "../mocks/").replace("dist/", "");
const loadMocksDirPath = path
    .join(__dirname, "../../../__tests__/snips/mocks")
    .replace("dist/", "");
export async function saveMock(options, result) {
    if (config.FIRECRAWL_SAVE_MOCKS !== true)
        return;
    await fs.mkdir(saveMocksDirPath, { recursive: true });
    const fileName = Date.now() + "-" + crypto.randomUUID() + ".json";
    const filePath = path.join(saveMocksDirPath, fileName);
    console.log(filePath);
    await fs.writeFile(filePath, JSON.stringify({
        time: Date.now(),
        options,
        result,
    }, undefined, 4));
}
export async function loadMock(name, logger = _logger) {
    try {
        const mockPath = path.join(loadMocksDirPath, name + ".json");
        const relative = path.relative(loadMocksDirPath, mockPath);
        if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
            // directory moving
            return null;
        }
        const load = JSON.parse(await fs.readFile(mockPath, "utf8"));
        return {
            requests: load,
            tracker: {},
        };
    }
    catch (error) {
        logger.warn("Failed to load mock file!", {
            name,
            module: "scrapeURL:mock",
            method: "loadMock",
            error,
        });
        return null;
    }
}
//# sourceMappingURL=mock.js.map