export function getAdjustedMaxDepth(url, maxCrawlDepth) {
    const baseURLDepth = getURLDepth(url);
    const adjustedMaxDepth = maxCrawlDepth + baseURLDepth;
    return adjustedMaxDepth;
}
export function getURLDepth(url) {
    const pathSplits = new URL(url).pathname
        .split("/")
        .filter(x => x !== "" && x !== "index.php" && x !== "index.html");
    return pathSplits.length;
}
//# sourceMappingURL=maxDepthUtils.js.map