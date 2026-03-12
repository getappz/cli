export class TransportableError extends Error {
    code;
    constructor(code, message, options) {
        super(message, options);
        this.code = code;
    }
}
//# sourceMappingURL=error.js.map