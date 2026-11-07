export class Cursor {
    public end: number;

    public constructor(end: number) {
        this.end = end;
    }

    public reserve(bytes: number, alignment: number): number {
        const start = Math.ceil(this.end / alignment) * alignment;
        this.end = start + bytes;
        if (!Number.isSafeInteger(this.end) || this.end > 0xffffffff || bytes < 0) {
            throw new RangeError("Intrlay buffer exceeds u32 capacity");
        }
        return start;
    }

    /** Rejects a source whose payload size changed between measurement and writing. */
    public finish(byteLength: number): void {
        if (this.end !== byteLength) {
            throw new RangeError("Intrlay layout changed during construction");
        }
    }
}

export function count(value: number | ArrayLike<unknown>): number {
    const length = typeof value === "number" ? value : value.length;
    if (!Number.isInteger(length) || length < 0 || length > 0xffffffff) {
        throw new RangeError("Intrlay element count exceeds u32 capacity");
    }
    return length;
}

