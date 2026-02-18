import * as undici from "undici";
export declare function fetchFileToBuffer(url: string, skipTlsVerification?: boolean, init?: undici.RequestInit): Promise<{
    response: undici.Response;
    buffer: Buffer;
}>;
export declare function downloadFile(id: string, url: string, skipTlsVerification?: boolean, init?: undici.RequestInit): Promise<{
    response: undici.Response;
    tempFilePath: string;
}>;
//# sourceMappingURL=downloadFile.d.ts.map